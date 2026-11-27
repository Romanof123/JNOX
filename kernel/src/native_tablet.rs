//! Modern virtio-input tablet on QEMU virt. The boot CPU owns transport, DMA
//! and reset in masked EL1. Both split queues suppress interrupts. The GIC
//! line remains masked; a reader polls ISR/used and acknowledges device IRQs.
use core::{arch::asm, mem, ptr};
use crate::context::{GICD, gic_read, gic_write};

#[path = "native_virtio_reset.rs"]
mod reset_protocol;

const N: usize = 16;
const BASE: usize = 0x0a00_0000;
const END: usize = BASE + 0x4000;
const ABS: u16 = 3;
const KEY: u16 = 1;
const LEFT: u16 = 272;

#[repr(C)]
#[derive(Clone, Copy)]
struct Desc { address: u64, length: u32, flags: u16, next: u16 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Completion { id: u32, length: u32 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Event { kind: u16, code: u16, value: i32 }
#[repr(C, align(4096))]
struct Queue {
    descriptors: [Desc; N],
    avail_flags: u16,
    avail_idx: u16,
    available: [u16; N],
    used_flags: u16,
    used_idx: u16,
    used: [Completion; N],
    events: [Event; N],
}
const _: () = assert!(mem::size_of::<Event>() == 8 && mem::offset_of!(Queue, used_flags) % 4 == 0);

#[unsafe(no_mangle)]
static mut jnox_tablet_queues: [Queue; 2] = unsafe { mem::zeroed() };

// [base, owner+1, phase (0 cold,1 live,2 retired,3 quarantined), x min/max,
// y min/max, avail cursor, used cursor, raw completions, SYN reports, resets].
#[unsafe(no_mangle)]
static mut jnox_tablet_state: [u64; 12] = [0; 12];
// [transport version, device id, offered features lo/hi, accepted lo/hi,
// physical INTID, distributor enabled bit, queue 0/1 max, ready status].
#[unsafe(no_mangle)]
static mut jnox_tablet_transport: [u64; 11] = [0; 11];
// Rolling history [sequence, raw little-endian event, descriptor id, length].
#[unsafe(no_mangle)]
static mut jnox_tablet_events: [[u64; 4]; 64] = [[0; 4]; 64];

struct Tablet {
    base: usize,
    owner: Option<usize>,
    broken: bool,
    axes: [(i32, i32); 2],
    used: u16,
    available: u16,
    pending: [u64; N],
    pos: usize,
    len: usize,
    sequence: u64,
}

static mut TABLET: Tablet = Tablet {
    base: 0, owner: None, broken: false, axes: [(0, 0); 2],
    used: 0, available: 0, pending: [0; N], pos: 0, len: 0, sequence: 0,
};

fn read(base: usize, reg: usize) -> u32 { unsafe { ((base + reg) as *const u32).read_volatile() } }
fn write(base: usize, reg: usize, value: u32) { unsafe { ((base + reg) as *mut u32).write_volatile(value) } }
fn fence() { unsafe { asm!("dmb sy", options(nostack, preserves_flags)) } }
fn evidence(index: usize, value: u64) {
    unsafe { (&raw mut jnox_tablet_state[index]).write_volatile(value) }
}
fn transport(index: usize, value: u32) {
    unsafe { (&raw mut jnox_tablet_transport[index]).write_volatile(value.into()) }
}

// QEMU virt allocates SPI (16 + transport slot) to the matching MMIO window.
fn mask_irq(base: usize) -> Result<(), i64> {
    let id = 48 + (base - BASE) / 0x200;
    let word = (id / 32) * 4;
    let bit = 1 << (id % 32);
    gic_write(GICD + 0x180 + word, bit);
    fence();
    let enabled = gic_read(GICD + 0x100 + word) & bit;
    transport(6, id as u32);
    transport(7, enabled);
    if enabled != 0 { return Err(-5); }
    gic_write(GICD + 0x280 + word, bit);
    fence();
    Ok(())
}
struct Transport(usize);
impl reset_protocol::ResetIo for Transport {
    fn request_reset(&mut self) { write(self.0, 0x70, 0); }
    fn status(&mut self) -> u32 { read(self.0, 0x70) }
    fn select(&mut self, queue: u32) { write(self.0, 0x30, queue); }
    fn ready(&mut self) -> u32 { read(self.0, 0x44) }
    fn fence(&mut self) { fence(); }
}

fn config(base: usize, select: u8, sub: u8) -> Result<([u8; 128], usize), i64> {
    unsafe {
        ((base + 0x100) as *mut u8).write_volatile(select);
        ((base + 0x101) as *mut u8).write_volatile(sub);
    }
    fence();
    for _ in 0..4 {
        let gen_before = read(base, 0xfc);
        fence();
        let size = unsafe { ((base + 0x102) as *const u8).read_volatile() } as usize;
        if size > 128 { return Err(-5); }
        let mut data = [0; 128];
        for (i, dest) in data[..size].iter_mut().enumerate() {
            *dest = unsafe { ((base + 0x108 + i) as *const u8).read_volatile() };
        }
        fence();
        if gen_before == read(base, 0xfc) { return Ok((data, size)); }
    }
    Err(-5)
}

fn identify(base: usize) -> Result<[(i32, i32); 2], i64> {
    let (abs, n) = config(base, 0x11, ABS as u8)?;
    if n == 0 || abs[0] & 3 != 3 { return Err(-19); }
    let (keys, n) = config(base, 0x11, KEY as u8)?;
    if n <= LEFT as usize / 8 || keys[LEFT as usize / 8] & (1 << (LEFT % 8)) == 0 { return Err(-19); }
    let mut axes = [(0, 0); 2];
    for (i, axis) in axes.iter_mut().enumerate() {
        let (bytes, size) = config(base, 0x12, i as u8)?;
        if size < 20 { return Err(-19); }
        *axis = (i32::from_le_bytes(bytes[..4].try_into().unwrap()), i32::from_le_bytes(bytes[4..8].try_into().unwrap()));
        if axis.0 >= axis.1 { return Err(-19); }
    }
    Ok(axes)
}

fn install_queue(base: usize, index: usize) -> Result<(), i64> {
    write(base, 0x30, index as u32);
    fence();
    let maximum = read(base, 0x34);
    transport(8 + index, maximum);
    if read(base, 0x44) != 0 || maximum < N as u32 { return Err(-5); }
    let q = unsafe { &raw mut jnox_tablet_queues[index] };
    unsafe {
        ptr::write_bytes(q, 0, 1);
        (*q).avail_flags = 1;
        if index == 0 {
            for id in 0..N {
                (*q).descriptors[id] = Desc {
                    address: (&raw mut (*q).events[id]) as u64, length: 8, flags: 2, next: 0,
                };
                (*q).available[id] = id as u16;
            }
            (*q).avail_idx = N as u16;
        }
        for (reg, address) in [
            (0x80, (&raw mut (*q).descriptors) as u64),
            (0x90, (&raw mut (*q).avail_flags) as u64),
            (0xa0, (&raw mut (*q).used_flags) as u64),
        ] {
            write(base, reg, address as u32);
            write(base, reg + 4, (address >> 32) as u32);
        }
    }
    write(base, 0x38, N as u32);
    fence();
    write(base, 0x44, 1);
    fence();
    if read(base, 0x44) != 1 { return Err(-5); }
    Ok(())
}

impl Tablet {
    fn configure(&mut self) -> Result<(), i64> {
        let b = self.base;
        mask_irq(b)?;
        transport(0, read(b, 4));
        transport(1, read(b, 8));
        reset_protocol::reset(&mut Transport(b), || {}).map_err(i64::from)?;
        write(b, 0x70, 3);
        write(b, 0x14, 0);
        fence();
        transport(2, read(b, 0x10));
        write(b, 0x14, 1);
        fence();
        let high = read(b, 0x10);
        transport(3, high);
        if high & 1 == 0 { return Err(-19); }
        for (page, bits) in [(0, 0), (1, 1)] {
            write(b, 0x24, page);
            write(b, 0x20, bits);
            transport(4 + page as usize, bits);
        }
        write(b, 0x70, 11);
        fence();
        if read(b, 0x70) != 11 { return Err(-5); }
        install_queue(b, 0)?;
        install_queue(b, 1)?;
        self.used = 0;
        self.available = N as u16;
        self.pos = 0;
        self.len = 0;
        write(b, 0x70, 15);
        fence();
        let status = read(b, 0x70);
        transport(10, status);
        if status != 15 { return Err(-5); }
        write(b, 0x50, 0);
        Ok(())
    }

    fn acquire(&mut self, pid: usize) -> Result<i64, i64> {
        if self.broken { return Err(-5); }
        if self.owner == Some(pid) { return Ok(0); }
        if self.owner.is_some() { return Err(-1); }
        let mut found = None;
        for b in (BASE..END).step_by(0x200) {
            if read(b, 0) != 0x7472_6976 || read(b, 4) != 2 || read(b, 8) != 18 { continue; }
            let axes = match identify(b) { Ok(a) => a, Err(-19) => continue, Err(e) => return Err(e) };
            if found.replace((b, axes)).is_some() { return Err(-19); }
        }
        let (b, axes) = found.ok_or(-19)?;
        self.base = b;
        self.axes = axes;
        evidence(0, b as u64);
        if let Err(e) = self.configure() { self.stop(true); return Err(e); }
        self.owner = Some(pid);
        evidence(1, pid as u64 + 1);
        evidence(2, 1);
        for (i, (min, max)) in axes.into_iter().enumerate() {
            evidence(3 + i * 2, i64::from(min) as u64);
            evidence(4 + i * 2, i64::from(max) as u64);
        }
        evidence(7, self.available.into());
        evidence(8, 0);
        Ok(0)
    }

    fn typed(&self, e: Event) -> Result<Option<u64>, i64> {
        let value = match (e.kind, e.code) {
            (0, 0) => 0,
            (0, 3) => return Err(-5), // SYN_DROPPED invalidates button and axis state
            (ABS, code @ 0..=1) => {
                let (lo, hi) = self.axes[code as usize];
                let delta = i64::from(e.value.clamp(lo, hi)) - i64::from(lo);
                (delta * if code == 0 { 319 } else { 239 } / (i64::from(hi) - i64::from(lo))) as u32
            }
            (KEY, LEFT) if matches!(e.value, 0 | 1) => e.value as u32,
            (KEY, LEFT) => return Err(-5),
            _ => return Ok(None),
        };
        Ok(Some((u64::from(e.kind) << 48) | (u64::from(e.code) << 32) | u64::from(value)))
    }

    fn refill(&mut self) -> Result<(), i64> {
        let irq = read(self.base, 0x60);
        if irq != 0 { write(self.base, 0x64, irq); }
        if irq & !1 != 0 || read(self.base, 0x70) != 15 { return Err(-5); }
        let q = unsafe { &raw mut jnox_tablet_queues[0] };
        let end = unsafe { (&raw const (*q).used_idx).read_volatile() };
        fence();
        let count = end.wrapping_sub(self.used) as usize;
        if count > N { return Err(-5); }
        self.pos = 0;
        self.len = 0;
        let mut seen = 0u32;
        for _ in 0..count {
            let used = unsafe { (&raw const (*q).used[self.used as usize % N]).read_volatile() };
            let id = used.id as usize;
            if id >= N || used.length != 8 || seen & (1 << id) != 0 { return Err(-5); }
            seen |= 1 << id;
            let ev = unsafe { (&raw const (*q).events[id]).read_volatile() };
            let raw = u64::from(ev.kind) | u64::from(ev.code) << 16 | u64::from(ev.value as u32) << 32;
            self.sequence += 1;
            unsafe {
                (&raw mut jnox_tablet_events[(self.sequence as usize - 1) % 64]).write_volatile([
                    self.sequence, raw, id as u64, used.length.into()
                ]);
            }
            evidence(9, self.sequence);
            if let Some(word) = self.typed(ev)? {
                if word == 0 {
                    unsafe { evidence(10, jnox_tablet_state[10] + 1); }
                }
                self.pending[self.len] = word;
                self.len += 1;
            }
            unsafe { (&raw mut (*q).available[self.available as usize % N]).write_volatile(id as u16); }
            self.used = self.used.wrapping_add(1);
            self.available = self.available.wrapping_add(1);
        }
        if count != 0 {
            fence();
            unsafe { (&raw mut (*q).avail_idx).write_volatile(self.available); }
            fence();
            write(self.base, 0x50, 0);
            evidence(7, self.available.into());
            evidence(8, self.used.into());
        }
        Ok(())
    }

    fn poll(&mut self, pid: usize) -> Result<i64, i64> {
        if self.broken { return Err(-5); }
        if self.owner != Some(pid) { return Err(-19); }
        if self.pos == self.len {
            if let Err(e) = self.refill() { self.stop(true); return Err(e); }
        }
        if self.pos == self.len { return Err(-11); }
        let word = self.pending[self.pos];
        self.pos += 1;
        Ok(word as i64)
    }

    fn stop(&mut self, fault: bool) {
        self.owner = None;
        self.pos = 0;
        self.len = 0;
        evidence(1, 0);
        let result = reset_protocol::reset(&mut Transport(self.base), || {
            unsafe { ptr::write_bytes((&raw mut jnox_tablet_queues).cast::<Queue>(), 0, 2); }
        });
        let masked = mask_irq(self.base);
        self.broken = fault || result.is_err() || masked.is_err();
        evidence(2, if self.broken { 3 } else { 2 });
        unsafe { evidence(11, jnox_tablet_state[11] + u64::from(result.is_ok())); }
    }
}

pub(super) fn call(pid: usize, op: u64) -> i64 {
    let tablet = unsafe { &mut *(&raw mut TABLET) };
    match op { 0 => tablet.acquire(pid), 1 => tablet.poll(pid), _ => Err(-22) }.unwrap_or_else(|e| e)
}

pub(super) fn retire(pid: usize) {
    let tablet = unsafe { &mut *(&raw mut TABLET) };
    if tablet.owner == Some(pid) { tablet.stop(false); }
}
