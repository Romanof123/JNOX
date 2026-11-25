//! Statically packaged native-v1 processes on a dedicated CPU0 virt boot.
use crate::native_elf::{self, LoadPlan, Permission};
use crate::native_el0_mmu::{self, STACK_BOTTOM, STACK_TOP};
use crate::ready_queue::ReadyQueue;
use crate::{context, park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

global_asm!(include_str!("native_el0_vectors.S"), options(raw));
unsafe extern "C" {
    fn native_el0_enter(entry: u64, sp: u64, spsr: u64) -> !;
    fn native_sleep_switch(idle_top: *mut u8);
    static native_el0_vectors: u8;
}

const ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));
#[used]
#[unsafe(no_mangle)]
static jnox_native_el0_image: [u8; ELF.len()] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));

#[cfg(jnox_native_peer)]
const PEER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_peer.elf"));

#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct NativeFrame {
    x: [u64; 31],
    esr: u64,
    spsr: u64,
    elr: u64,
    sp_el0: u64,
    current_el: u64,
    fpcr: u64,
    fpsr: u64,
    q: [[u64; 2]; 32],
    tls: [u64; 2],
}
const _: () = assert!(size_of::<NativeFrame>() == 832
    && offset_of!(NativeFrame, esr) == 248 && offset_of!(NativeFrame, spsr) == 256
    && offset_of!(NativeFrame, elr) == 264 && offset_of!(NativeFrame, sp_el0) == 272
    && offset_of!(NativeFrame, current_el) == 280 && offset_of!(NativeFrame, q) == 304);

#[repr(C)]
struct Journal {
    header: [u64; 16],
    rows: [[u64; 16]; 8],
}
#[unsafe(no_mangle)]
static mut jnox_native_el0_journal: Journal = Journal { header: [0; 16], rows: [[0; 16]; 8] };

fn head(index: usize, value: u64) {
    // One CPU. Boot and the EL1 vectors are masked and never nested.
    unsafe { ptr::write_volatile((&raw mut jnox_native_el0_journal.header).cast::<u64>().add(index), value) };
}

// Initialized once on this masked boot path, before the first EL0 entry.
static mut CLOCK_DOMAIN: (u64, u64) = (0, 0);

fn counter() -> u64 {
    let ticks: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) ticks, options(nostack, preserves_flags)) };
    ticks
}

fn clock_gettime(clock_id: u64, address: usize) -> i64 {
    if clock_id != 1 {
        return -22;
    }
    let (epoch, frequency) = unsafe { CLOCK_DOMAIN };
    let Some(elapsed) = counter().checked_sub(epoch) else { return -75 };
    let ns = u128::from(elapsed) * 1_000_000_000 / u128::from(frequency);
    let Ok(seconds) = i64::try_from(ns / 1_000_000_000) else { return -75 };
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&seconds.to_le_bytes());
    bytes[8..].copy_from_slice(&((ns % 1_000_000_000) as i64).to_le_bytes());
    if native_el0_mmu::copy_to_user(address, &bytes) { 0 } else { -14 }
}

#[derive(Clone, Copy, PartialEq)]
enum TaskState { Absent, Ready, Running, Sleeping(u64), Exited(i32) }

const INITIAL_PSTATE: u64 = if cfg!(jnox_native_peer) { 0x340 } else { 0x3c0 };
const EMPTY_FRAME: NativeFrame = NativeFrame {
    x: [0; 31], esr: 0, spsr: INITIAL_PSTATE, elr: 0, sp_el0: 0,
    current_el: 4, fpcr: 0, fpsr: 0, q: [[0; 2]; 32], tls: [0; 2],
};

// CPU0 masks IRQs on every scheduler entry. The idle continuation keeps no
// reference into scheduler storage across the interval with IRQs enabled.
static mut FRAMES: [NativeFrame; 2] = [EMPTY_FRAME; 2];
static mut STATES: [TaskState; 2] = [TaskState::Running, TaskState::Absent];
static mut READY: ReadyQueue<2> = ReadyQueue::new();
static mut CURRENT: usize = 0;
static mut IDLE: context::Stack = context::Stack([0; context::STACK_BYTES]);

fn enqueue(pid: usize) {
    unsafe {
        if !(*(&raw mut READY)).push_back(pid) { fail(9) }
        STATES[pid] = TaskState::Ready;
    }
}

fn update_timer(now: u64) -> bool {
    let mut earliest: Option<u64> = None;
    let mut woke = false;
    for (pid, state) in unsafe { STATES }.into_iter().enumerate() {
        if let TaskState::Sleeping(target) = state {
            if target <= now {
                enqueue(pid);
                woke = true;
            } else {
                earliest = Some(earliest.map_or(target, |prior| prior.min(target)));
            }
        }
    }
    unsafe {
        if let Some(target) = earliest {
            asm!("msr cntp_cval_el0, {target}", "msr cntp_ctl_el0, {enable}", "isb",
                 target = in(reg) target, enable = in(reg) 1u64, options(nostack, preserves_flags));
        } else {
            asm!("msr cntp_ctl_el0, xzr", "isb", options(nostack, preserves_flags));
        }
    }
    woke
}

fn sleep_until(f: &NativeFrame) -> Result<Option<u64>, i64> {
    if f.x[0] != 1 || f.x[1] != 1 {
        return Err(-22);
    }
    let mut request = [0u8; 16];
    if !native_el0_mmu::copy_from_user(f.x[2] as usize, &mut request) {
        return Err(-14);
    }
    let seconds = i64::from_le_bytes(request[..8].try_into().unwrap());
    let nanos = i64::from_le_bytes(request[8..].try_into().unwrap());
    if seconds < 0 || !(0..1_000_000_000).contains(&nanos) {
        return Err(-22);
    }
    let (epoch, frequency) = unsafe { CLOCK_DOMAIN };
    let hz = u128::from(frequency);
    // Signed LP64 seconds times an unsigned 64-bit frequency fits u128,
    // including the subsecond contribution and the 64-bit epoch.
    let ticks = u128::from(epoch) + seconds as u128 * hz
        + (nanos as u128 * hz).div_ceil(1_000_000_000);
    let Ok(target) = u64::try_from(ticks) else { return Err(-75) };
    Ok((counter() < target).then_some(target))
}

fn dispatch(frame: &mut NativeFrame) {
    let prior = unsafe { CURRENT };
    unsafe { FRAMES[prior] = *frame };
    loop {
        update_timer(counter());
        if let Some(next) = unsafe { (*(&raw mut READY)).pop_highest(|_| 0) } {
            unsafe {
                if next != prior {
                    native_el0_mmu::switch(next).unwrap_or_else(|_| fail(9));
                    *frame = FRAMES[next];
                }
                CURRENT = next;
                STATES[next] = TaskState::Running;
            }
            if next != prior { event(b"DISPATCH", next, None); }
            return;
        }
        if !unsafe { STATES }.iter().any(|s| matches!(s, TaskState::Sleeping(_))) {
            head(10, 1);
            head(1, 3);
            unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
            park()
        }
        let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
        unsafe { native_sleep_switch(top) };
    }
}

fn event(kind: &[u8], pid: usize, value: Option<i64>) {
    if cfg!(jnox_native_peer) {
        uart_write(b"NATIVE EL0 ");
        uart_write(kind);
        uart_write(b" PID ");
        decimal(pid as i64);
        if let Some(value) = value {
            uart_write(b" ");
            decimal(value);
        }
        uart_write(b"\n");
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_sleep_irq() -> u64 {
    let iar = context::gic_read(context::GICC + 0x0c);
    let id = iar & 0x3ff;
    if id >= 1020 { return 0 }
    let ready = id == context::TIMER_IRQ && update_timer(counter());
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    context::gic_write(context::GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    ready as u64
}

fn decimal(value: i64) {
    if value < 0 {
        uart_write(b"-");
    }
    let mut n = value.unsigned_abs();
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&digits[i..]);
}

fn fail(code: u64) -> ! {
    head(2, code);
    head(1, 4);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    uart_write(b"NATIVE EL0 FAULT ");
    decimal(code as i64);
    uart_write(b"\n");
    park()
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_unexpected() -> ! {
    fail(1)
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_sync(frame: *mut NativeFrame, vector: u64) {
    // The lower-A64 vector created this private, 16-byte aligned EL1 frame.
    let f = unsafe { &mut *frame };
    let vbar: u64;
    let ttbr: u64;
    let far: u64;
    unsafe {
        asm!("mrs {}, vbar_el1", "mrs {}, ttbr0_el1", "mrs {}, far_el1",
            out(reg) vbar, out(reg) ttbr, out(reg) far, options(nostack, preserves_flags));
    }
    let count = unsafe { ptr::read_volatile(&raw const jnox_native_el0_journal.header[8]) as usize };
    if vector != 0x400 || vbar != (&raw const native_el0_vectors).addr() as u64
        || f.current_el != 4 || f.spsr & 0x1f != 0 || f.esr >> 26 != 0x15
        || f.esr & 0xffff != 0x4a01 || !native_el0_mmu::user_range(f.elr as usize, 4, false)
        || !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&f.sp_el0) || f.sp_el0 % 16 != 0
    {
        fail(2);
    }
    let exit = f.x[8] == 1;
    if exit && f.x[0] as i32 as i64 as u64 != f.x[0] {
        fail(3);
    }
    // Unsupported services do not dereference a user pointer, acquire
    // descriptors, emit output or claim to have completed a transfer.
    let mut deadline = None;
    let result = match f.x[8] {
        1 => 0,
        6 => clock_gettime(f.x[0], f.x[1] as usize),
        7 => match sleep_until(f) {
            Ok(target) => { deadline = target; 0 }
            Err(error) => error,
        },
        8 if cfg!(jnox_native_process) => 0,
        _ => -38,
    };
    // Keep the first seven calls and the most recent call once the journal fills.
    let index = count.min(7);
    let row = [index as u64, vector, f.current_el, f.esr, f.spsr, f.elr, f.sp_el0,
        f.x[8], f.x[0], f.x[1], f.x[2], result as u64, if exit { 2 } else { 1 }, vbar, ttbr, far];
    unsafe {
        let out = (&raw mut jnox_native_el0_journal.rows).cast::<[u64; 16]>().add(index);
        ptr::write_volatile(out, row);
        asm!("dmb ishst", options(nostack, preserves_flags));
    }
    head(8, index as u64 + 1);
    let pid = unsafe { CURRENT };
    if exit {
        let status = f.x[0] as i32;
        head(9, f.x[0]);
        unsafe { STATES[pid] = TaskState::Exited(status) };
        event(b"EXIT", pid, Some(i64::from(status)));
        if !cfg!(jnox_native_peer) {
            head(10, 1);
            head(1, 3);
            unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
            uart_write(b"NATIVE EL0 EXIT ");
            decimal(i64::from(status));
            uart_write(b"\n");
        }
        dispatch(f);
    } else {
        f.x[0] = result as u64;
        if let Some(target) = deadline {
            unsafe { STATES[pid] = TaskState::Sleeping(target) };
            event(b"SLEEP", pid, None);
            dispatch(f);
        } else if f.x[8] == 8 && result == 0 {
            update_timer(counter());
            enqueue(pid);
            dispatch(f);
        }
    }
}

// The bootstrap source is fixed. The ordinary argument strings and pointers
// are copied to private physical backing before the user mapping is enabled.
#[cfg(jnox_native_process)]
const ARGS: &[&[u8]] = &[b"/bin/app\0"];
#[cfg(not(jnox_native_process))]
const ARGS: &[&[u8]] = &[b"/bin/native_startup_probe\0", b"8\0", b"13\0"];

fn initial_stack() -> [u8; 80] {
    let mut tail = [0u8; 80];
    let sp = STACK_TOP - tail.len();
    tail[..8].copy_from_slice(&(ARGS.len() as u64).to_le_bytes());
    let mut offset = 48;
    for (i, arg) in ARGS.iter().enumerate() {
        let pointer = ((sp + offset) as u64).to_le_bytes();
        tail[8 * (i + 1)..8 * (i + 2)].copy_from_slice(&pointer);
        tail[offset..offset + arg.len()].copy_from_slice(arg);
        offset += arg.len();
    }
    tail
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn section_bytes<'a>(image: &'a [u8], section: &[u8]) -> Option<&'a [u8]> {
    let start = u64_at(section, 24)? as usize;
    let len = u64_at(section, 32)? as usize;
    image.get(start..start.checked_add(len)?)
}

/// The generic ELF plan covers loadable segments. Independently bind the
/// exact allocated SHT_NOTE to the native v1 SDK personality and its RO bytes.
fn native_v1_note(image: &[u8], plan: &LoadPlan<'_>) -> Option<()> {
    const NOTE: &[u8] = b"\x05\0\0\0\x04\0\0\0\x01\0\0\0JNOX\0\0\0\0\x01\0\0\0";
    let mut sizes = image.get(58..64)?.chunks_exact(2);
    let mut word = || Some(usize::from(u16::from_le_bytes(sizes.next()?.try_into().ok()?)));
    let stride = word()?;
    let count = word()?;
    let names_id = word()?;
    if stride != 64 || !(1..=128).contains(&count) || names_id >= count {
        return None;
    }
    let start = usize::try_from(u64_at(image, 40)?).ok()?;
    let table = image.get(start..start.checked_add(count * stride)?)?;
    let names_header = &table[names_id * stride..(names_id + 1) * stride];
    if u32_at(names_header, 4)? != 3 {
        return None;
    }
    let names = section_bytes(image, names_header)?;
    let mut found = false;
    for sh in table.chunks_exact(stride) {
        let tail = names.get(u32_at(sh, 0)? as usize..)?;
        let name = tail.get(..tail.iter().position(|&b| b == 0)?)?;
        if name != b".note.jnox.abi" {
            continue;
        }
        if found || u32_at(sh, 4)? != 7 || u64_at(sh, 8)? != 2 || section_bytes(image, sh)? != NOTE {
            return None;
        }
        let va = u64_at(sh, 16)?;
        let file = u64_at(sh, 24)?;
        found = plan.segments().iter().any(|s| {
            let Some(delta) = va.checked_sub(s.address) else { return false };
            s.permission == Permission::Read && s.file_offset.checked_add(delta) == Some(file)
                && delta.checked_add(NOTE.len() as u64).is_some_and(|end| end <= s.bytes.len() as u64)
        });
        if !found {
            return None;
        }
    }
    found.then_some(())
}

fn validated_image(image: &[u8]) -> LoadPlan<'_> {
    let plan = native_elf::plan(image, 0x0040_0000..0x0080_0000)
        .unwrap_or_else(|_| fail(4));
    let perms = [Permission::ReadExecute, Permission::Read, Permission::ReadWrite];
    if plan.entry != 0x0040_0000 || !plan.segments().iter().map(|s| s.permission).eq(perms)
        || native_v1_note(image, &plan).is_none()
    {
        fail(5);
    }
    plan
}

pub(super) fn run() -> ! {
    let frequency: u64;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags));
        if frequency == 0 {
            fail(8);
        }
        CLOCK_DOMAIN = (counter(), frequency);
    }
    head(0, 1);
    head(1, 1);
    head(3, jnox_native_el0_image.len() as u64);
    uart_write(b"NATIVE EL0 ELF ");
    uart_write(env!("JNOX_PACKAGED_ELF_SHA256").as_bytes());
    uart_write(b"\n");
    let plan = validated_image(&jnox_native_el0_image);
    #[cfg(jnox_native_peer)]
    let peer = {
        uart_write(b"NATIVE EL0 PEER ELF ");
        uart_write(env!("JNOX_PACKAGED_PEER_SHA256").as_bytes());
        uart_write(b"\n");
        validated_image(PEER)
    };
    let tail = initial_stack();
    let sp = STACK_TOP - tail.len();
    // Dedicated virt boot with a quiescent single CPU. Other profiles keep
    // their own interrupt and timer setup.
    unsafe {
        asm!("msr daifset, #0xf", "msr cntp_ctl_el0, xzr", "msr cntv_ctl_el0, xzr",
            "isb", options(nostack, preserves_flags));
        ptr::write_volatile(0x0801_0000 as *mut u32, 0);
        ptr::write_volatile(0x0800_0000 as *mut u32, 0);
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
        context::configure_timer_ppi();
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
        #[cfg(jnox_native_peer)]
        {
            native_el0_mmu::install_pair([&plan, &peer], [&tail, &tail]).unwrap_or_else(|_| fail(6));
            FRAMES[1].elr = peer.entry;
            FRAMES[1].sp_el0 = sp as u64;
            enqueue(1);
        }
        #[cfg(not(jnox_native_peer))]
        native_el0_mmu::install(&plan, &tail).unwrap_or_else(|_| fail(6));
    }
    let current: u64;
    let sctlr: u64;
    let ttbr: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", "mrs {}, sctlr_el1", "mrs {}, ttbr0_el1",
            out(reg) current, out(reg) sctlr, out(reg) ttbr, options(nostack, preserves_flags));
    }
    if current != 4 || sctlr & 1 == 0 || !native_el0_mmu::user_range(sp, tail.len(), true) {
        fail(7);
    }
    head(4, plan.entry);
    head(5, sp as u64);
    head(6, STACK_BOTTOM as u64);
    head(7, STACK_TOP as u64);
    head(12, current);
    head(13, sctlr);
    head(14, ttbr);
    head(15, ARGS.len() as u64);
    head(1, 2);
    event(b"DISPATCH", 0, None);
    unsafe { native_el0_enter(plan.entry, sp as u64, INITIAL_PSTATE) }
}
