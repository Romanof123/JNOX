//! Polling modern virtio-net MMIO for the single-CPU QEMU virt native profile.
//! The four MMIO pages are EL1 Device memory. Fixed queue storage is in the
//! EL1-only identity-mapped, Normal non-cacheable BSS (no IOMMU or caches).
//! See Virtio 1.2 sections 2.7 (split queues), 4.2 (MMIO) and 5.1 (network).
use core::{arch::asm, mem::offset_of, ptr};

const MMIO_START: usize = 0x0a00_0000;
const MMIO_END: usize = 0x0a00_4000;
const MAC: [u8; 6] = [0x52, 0x54, 0, 0x12, 0x34, 0x56];
const SIZE: usize = 8;
const RX: usize = 0;
const TX: usize = 1;
const HEADER: usize = 12; // VERSION_1 includes num_buffers without MRG_RXBUF.
const MIN_FRAME: usize = 14;
pub(crate) const MAX_FRAME: usize = 1514;
const BUFFER: usize = HEADER + MAX_FRAME;

const MAGIC: usize = 0x00;
const VERSION: usize = 0x04;
const DEVICE_ID: usize = 0x08;
const FEATURES: usize = 0x10;
const FEATURES_SEL: usize = 0x14;
const DRIVER_FEATURES: usize = 0x20;
const DRIVER_FEATURES_SEL: usize = 0x24;
const QUEUE_SEL: usize = 0x30;
const QUEUE_MAX: usize = 0x34;
const QUEUE_NUM: usize = 0x38;
const QUEUE_READY: usize = 0x44;
const NOTIFY: usize = 0x50;
const INTERRUPT_STATUS: usize = 0x60;
const INTERRUPT_ACK: usize = 0x64;
const STATUS: usize = 0x70;
const QUEUE_DESC: usize = 0x80;
const QUEUE_DRIVER: usize = 0x90;
const QUEUE_DEVICE: usize = 0xa0;
const CONFIG_GEN: usize = 0xfc;
const CONFIG: usize = 0x100;
const ACK_DRIVER: u32 = 1 | 2;
const FEATURES_OK: u32 = 8;
const DRIVER_OK: u32 = 4;
const FAILED: u32 = 128;
const READY: u32 = ACK_DRIVER | FEATURES_OK | DRIVER_OK;
const MAC_FEATURE: u32 = 1 << 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}
#[repr(C)]
struct Available {
    flags: u16,
    idx: u16,
    ring: [u16; SIZE],
    event: u16,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct UsedEntry {
    id: u32,
    len: u32,
}
#[repr(C)]
struct Used {
    flags: u16,
    idx: u16,
    ring: [UsedEntry; SIZE],
    event: u16,
}
#[repr(C, align(16))]
struct Frame([u8; BUFFER]);
#[repr(C, align(4096))]
struct Queue {
    desc: [Descriptor; SIZE],
    avail: Available,
    used: Used,
    frames: [Frame; SIZE],
}

const _: () = assert!(
    core::mem::size_of::<Descriptor>() == 16
        && SIZE.is_power_of_two()
        && SIZE <= 16
        && offset_of!(Queue, avail) % 2 == 0
        && offset_of!(Queue, used) % 4 == 0
);
// All fields are integer-only; BSS is zeroed before this once-only install.
#[unsafe(no_mangle)]
static mut jnox_native_net_queues: [Queue; 2] = unsafe { core::mem::zeroed() };

#[derive(Clone, Copy, Default)]
pub(crate) struct Stats {
    pub base: u64,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub tx_completed: u64,
    pub rx_empty: u64,
    pub tx_full: u64,
    pub faults: u64,
}

#[derive(Clone, Copy)]
struct Cursor {
    available: u16,
    used: u16,
    device_owned: u16,
}

struct Driver {
    base: usize,
    failed: bool,
    cursors: [Cursor; 2],
    received: [UsedEntry; SIZE],
    head: usize,
    count: usize,
    stats: Stats,
}

// Zero is the cold state for every data field. This object is never given to
// the device. All entries, including terminal reporting, run with IRQs masked.
static mut DRIVER: Driver = unsafe { core::mem::zeroed() };

fn serialized<T>(op: impl FnOnce(&mut Driver) -> T) -> T {
    let daif: u64;
    unsafe {
        asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags));
    }
    assert!(daif & (1 << 7) != 0);
    unsafe { op(&mut *(&raw mut DRIVER)) }
}

fn read(base: usize, off: usize) -> u32 {
    unsafe { ((base + off) as *const u32).read_volatile() }
}
fn write(base: usize, off: usize, val: u32) {
    unsafe { ((base + off) as *mut u32).write_volatile(val) }
}
fn barrier() {
    unsafe {
        asm!("dmb sy", options(nostack, preserves_flags));
    }
}
fn queue(q: usize) -> *mut Queue {
    unsafe { &raw mut jnox_native_net_queues[q] }
}
fn buffer(q: usize, id: usize) -> *mut u8 {
    unsafe { (&raw mut (*queue(q)).frames[id]).cast::<u8>() }
}
fn set_address(base: usize, lo_reg: usize, address: usize) {
    write(base, lo_reg, address as u32);
    write(base, lo_reg + 4, (address >> 32) as u32);
}

fn correct_mac(base: usize) -> bool {
    // The config generation must cover the whole multi-byte read.
    for _ in 0..4 {
        let generation = read(base, CONFIG_GEN);
        barrier();
        let observed: [u8; 6] =
            core::array::from_fn(|i| unsafe { ((base + CONFIG + i) as *const u8).read_volatile() });
        barrier();
        if read(base, CONFIG_GEN) == generation {
            return observed == MAC;
        }
    }
    false
}

impl Driver {
    fn setup(&mut self) -> Result<(), i32> {
        let mut device = None;
        for base in (MMIO_START..MMIO_END).step_by(0x200) {
            if read(base, MAGIC) == 0x7472_6976 && read(base, VERSION) == 2 && read(base, DEVICE_ID) == 1 {
                if device.replace(base).is_some() {
                    return Err(-19);
                }
            }
        }
        let base = device.ok_or(-19)?;
        self.base = base;
        self.stats.base = base as u64;
        write(base, STATUS, 0);
        if !(0..1024).any(|_| read(base, STATUS) == 0) {
            return Err(-5);
        }
        write(base, STATUS, 1);
        write(base, STATUS, ACK_DRIVER);
        // Advertise exactly MAC (bank 0 bit 5) and VERSION_1 (bank 1 bit 0).
        for (bank, required) in [(0, MAC_FEATURE), (1, 1)] {
            write(base, FEATURES_SEL, bank);
            if read(base, FEATURES) & required != required {
                return Err(-19);
            }
            write(base, DRIVER_FEATURES_SEL, bank);
            write(base, DRIVER_FEATURES, required);
        }
        write(base, STATUS, ACK_DRIVER | FEATURES_OK);
        if read(base, STATUS) != (ACK_DRIVER | FEATURES_OK) || !correct_mac(base) {
            return Err(-19);
        }

        for q in [RX, TX] {
            write(base, QUEUE_SEL, q as u32);
            if read(base, QUEUE_READY) != 0 || read(base, QUEUE_MAX) < SIZE as u32 {
                return Err(-19);
            }
            let mem = queue(q);
            unsafe {
                // VRING_AVAIL_F_NO_INTERRUPT. We poll and acknowledge advisory
                // interrupt status; this profile does not enable device SPIs.
                (&raw mut (*mem).avail.flags).write_volatile(1);
                for id in 0..SIZE {
                    let descriptor = Descriptor {
                        addr: buffer(q, id) as u64,
                        len: BUFFER as u32,
                        flags: if q == RX { 2 } else { 0 },
                        next: 0,
                    };
                    (&raw mut (*mem).desc[id]).write_volatile(descriptor);
                }
                barrier();
                write(base, QUEUE_NUM, SIZE as u32);
                set_address(base, QUEUE_DESC, (&raw mut (*mem).desc) as usize);
                set_address(base, QUEUE_DRIVER, (&raw mut (*mem).avail) as usize);
                set_address(base, QUEUE_DEVICE, (&raw mut (*mem).used) as usize);
            }
            write(base, QUEUE_READY, 1);
            if read(base, QUEUE_READY) != 1 {
                return Err(-5);
            }
        }
        for id in 0..SIZE {
            self.post(RX, id);
        }
        barrier();
        write(base, STATUS, READY);
        if read(base, STATUS) != READY {
            return Err(-5);
        }
        write(base, NOTIFY, RX as u32);
        Ok(())
    }

    // Latch faults. No later call reuses the rings or any DMA-owned buffer.
    fn fault(&mut self) {
        self.failed = true;
        self.stats.faults = self.stats.faults.saturating_add(1);
        if self.base != 0 {
            write(self.base, STATUS, read(self.base, STATUS) | FAILED);
            barrier();
        }
    }

    fn post(&mut self, q: usize, id: usize) {
        let cursor = &mut self.cursors[q];
        let index = cursor.available as usize % SIZE;
        cursor.device_owned |= 1 << id;
        cursor.available = cursor.available.wrapping_add(1);
        let mem = queue(q);
        unsafe {
            if q == RX {
                // The non-mergeable layout always has one descriptor. Some
                // QEMU backends fill only the first ten bytes of the header.
                buffer(q, id).add(10).write(1);
                buffer(q, id).add(11).write(0);
            }
            (&raw mut (*mem).avail.ring[index]).write_volatile(id as u16);
            barrier(); // payload, descriptor and ring entry before avail.idx
            (&raw mut (*mem).avail.idx).write_volatile(cursor.available);
            barrier(); // avail.idx before doorbell
        }
    }

    fn harvest(&mut self, q: usize) -> Result<(), i32> {
        let mem = queue(q);
        let cursor = &mut self.cursors[q];
        let end = unsafe { (&raw const (*mem).used.idx).read_volatile() };
        let count = end.wrapping_sub(cursor.used);
        if u32::from(count) > cursor.device_owned.count_ones() {
            return Err(-5);
        }
        barrier(); // observe used.idx before the corresponding entries and data
        for _ in 0..count {
            let entry = unsafe { (&raw const (*mem).used.ring[cursor.used as usize % SIZE]).read_volatile() };
            let id = entry.id as usize;
            if id >= SIZE || cursor.device_owned & (1 << id) == 0 {
                return Err(-5);
            }
            if q == RX {
                if !(HEADER + MIN_FRAME..=BUFFER).contains(&(entry.len as usize)) || self.count >= SIZE {
                    return Err(-5);
                }
                let p = buffer(q, id);
                unsafe {
                    if p.read_volatile() != 0
                        || p.add(1).read_volatile() != 0
                        || p.add(10).read_volatile() != 1
                        || p.add(11).read_volatile() != 0
                    {
                        return Err(-5);
                    }
                }
                self.received[(self.head + self.count) % SIZE] = entry;
                self.count += 1;
            } else {
                if entry.len != 0 {
                    return Err(-5);
                } // TX descriptors are device-readable only.
                self.stats.tx_completed = self.stats.tx_completed.saturating_add(1);
            }
            cursor.device_owned &= !(1 << id);
            cursor.used = cursor.used.wrapping_add(1);
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<(), i32> {
        if self.failed {
            return Err(-5);
        }
        if self.base == 0 {
            return Err(-19);
        }
        let interrupts = read(self.base, INTERRUPT_STATUS) & 3;
        if read(self.base, STATUS) != READY
            || (interrupts & 2 != 0 && !correct_mac(self.base))
            || self.harvest(TX).is_err()
            || self.harvest(RX).is_err()
        {
            self.fault();
            return Err(-5);
        }
        if interrupts != 0 {
            write(self.base, INTERRUPT_ACK, interrupts);
        }
        Ok(())
    }
}

pub(crate) fn init() -> Result<(), i32> {
    serialized(|d| {
        if d.failed {
            return Err(-5);
        }
        if d.base != 0 {
            return Ok(());
        }
        let result = d.setup();
        if result.is_err() {
            d.fault();
        }
        result
    })
}

pub(crate) fn rx(dst: &mut [u8]) -> Result<usize, i32> {
    serialized(|d| {
        d.poll()?;
        if d.count == 0 {
            d.stats.rx_empty = d.stats.rx_empty.saturating_add(1);
            return Err(-11);
        }
        let entry = d.received[d.head];
        let len = entry.len as usize - HEADER;
        if dst.len() < len {
            return Err(-90);
        } // retain this exact frame
        let id = entry.id as usize;
        unsafe {
            ptr::copy_nonoverlapping(buffer(RX, id).add(HEADER), dst.as_mut_ptr(), len);
        }
        d.head = (d.head + 1) % SIZE;
        d.count -= 1;
        d.post(RX, id);
        write(d.base, NOTIFY, RX as u32);
        d.stats.rx_frames = d.stats.rx_frames.saturating_add(1);
        Ok(len)
    })
}

pub(crate) fn tx(frame: &[u8]) -> Result<usize, i32> {
    if !(MIN_FRAME..=MAX_FRAME).contains(&frame.len()) {
        return Err(-90);
    }
    serialized(|d| {
        d.poll()?;
        let Some(id) = (0..SIZE).find(|&i| d.cursors[TX].device_owned & (1 << i) == 0) else {
            d.stats.tx_full = d.stats.tx_full.saturating_add(1);
            return Err(-11);
        };
        let p = buffer(TX, id);
        unsafe {
            ptr::write_bytes(p, 0, HEADER);
            ptr::copy_nonoverlapping(frame.as_ptr(), p.add(HEADER), frame.len());
            (&raw mut (*queue(TX)).desc[id].len).write_volatile((HEADER + frame.len()) as u32);
        }
        d.post(TX, id);
        write(d.base, NOTIFY, TX as u32);
        d.stats.tx_frames = d.stats.tx_frames.saturating_add(1);
        Ok(frame.len())
    })
}

pub(crate) fn stats() -> Stats {
    serialized(|d| {
        // Capture completions already posted since the last TX syscall.
        if d.base != 0 && !d.failed {
            let _ = d.poll();
        }
        d.stats
    })
}
