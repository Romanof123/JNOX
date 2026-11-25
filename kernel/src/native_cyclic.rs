//! Anchored, non-borrowing partition windows on CPU0.
use super::*;
use crate::temporal_policy::NativeCyclic;

const IDLE_OWNER: usize = 2;
const TERMINAL: usize = 3;
const LIMIT: usize = 512;
const WIDTH: usize = 33;
static mut ROWS: [[u64; WIDTH]; LIMIT] = [[0; WIDTH]; LIMIT];
static mut STATE: Option<Runtime> = None;
// A masked final ERET stamp uses only two scratch registers, saved on EL1 SP.
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = core::ptr::null_mut();
unsafe extern "C" { static native_cyclic_idle_landing: u8; }

struct Runtime {
    table: NativeCyclic,
    epoch: u64,
    hz: u64,
    owner: usize,
    compare: u64,
    sequence: Option<u64>,
    end: u64,
    wake: [Option<u64>; 2],
    exited: u8,
    used: usize,
    lost: u64,
    current_row: Option<usize>,
}

fn state() -> &'static mut Runtime { unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(21)) } }
fn idle_top() -> *mut u8 { (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES) }
fn root() -> u64 {
    let value;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack, preserves_flags)); }
    value
}

pub(super) fn start(hz: u64) -> usize {
    let now = counter();
    let epoch = now.checked_add(hz / 10).unwrap_or_else(|| fail(21));
    let table = NativeCyclic::new(epoch, hz, NATIVE_PARTITION_WINDOWS).unwrap_or_else(|| fail(21));
    unsafe {
        STATE = Some(Runtime {
            table, epoch, hz, owner: IDLE_OWNER, compare: 0, sequence: None, end: epoch,
            wake: [None; 2], exited: 0, used: 0, lost: 0, current_row: None,
        });
        asm!("msr vbar_el1, {vectors}", "msr cntkctl_el1, {access}", "isb",
             vectors=in(reg) &raw const native_el0_vectors, access=in(reg) 1u64, options(nostack));
    }
    let stamp = counter();
    if stamp >= epoch { fail(21); }
    select(stamp, 0, 0, 0, None);
    unsafe { native_sleep_switch(idle_top()); }
    state().owner
}

fn select(now: u64, kind: u64, op: u64, arg: u64, f: Option<&NativeFrame>) -> usize {
    let r = state();
    let prior = r.owner;
    let mut ready = 0;
    let mut woke = 0u8;
    for (id, wake) in r.wake.iter_mut().enumerate() {
        if wake.is_some_and(|at| at <= now) { *wake = None; woke |= 1 << id; }
        if wake.is_none() && r.exited & (1 << id) == 0 { ready |= 1 << id; }
    }
    let w = r.table.at(now, ready).unwrap_or_else(|_| fail(21));
    let next = if r.exited == 3 { TERMINAL } else { w.and_then(|v| v.runnable).unwrap_or(IDLE_OWNER) };
    let edge = w.map_or(r.epoch, |v| v.end);
    let target = r.wake.iter().flatten().copied().fold(edge, u64::min);
    arm_compare((next != TERMINAL).then_some(target));
    let seq = w.map(|v| v.sequence);
    let skipped = seq.map_or(0, |s| match r.sequence { Some(prev) => s.saturating_sub(prev).saturating_sub(1), None => s });
    let dst = if next < IDLE_OWNER {
        if prior == next && f.is_some() { f } else { Some(unsafe { &*(&raw const FRAMES[next]) }) }
    } else { None };
    let (src_pc, src_sp) = if let Some(f) = f { (f.elr, f.sp_el0) } else if kind == 1 {
        let pc;
        unsafe { asm!("mrs {}, elr_el1", out(reg) pc, options(nostack, preserves_flags)); }
        // IRQs are enabled only at the stackless WFI loop, after the idle observation.
        (pc, idle_top() as u64)
    } else { (0, 0) };
    let (dst_pc, dst_sp) = dst.map_or_else(|| if next == IDLE_OWNER {
        (&raw const native_cyclic_idle_landing as u64, idle_top() as u64)
    } else { (0, 0) }, |f| (f.elr, f.sp_el0));
    if r.used < LIMIT {
        let index = r.used;
        unsafe { ROWS[index] = [
            index as u64, kind, now, op, prior as u64, next as u64,
            r.sequence.unwrap_or(NO_PID), seq.unwrap_or(NO_PID), w.map_or(NO_PID, |v| v.frame),
            w.map_or(NO_PID, |v| v.slot as u64), w.and_then(|v| v.owner).unwrap_or(IDLE_OWNER) as u64,
            w.map_or(0, |v| v.start), edge, r.compare, if next == TERMINAL { 0 } else { target },
            ready as u64, woke as u64, r.wake[0].unwrap_or(0), r.wake[1].unwrap_or(0), r.exited as u64,
            skipped, now.saturating_sub(r.end), src_pc, src_sp, root(), dst_pc, dst_sp, 0, 0, 0,
            arg, f.map_or(0, |f| f.x[0]), counter(),
        ]; }
        r.current_row = Some(index);
        r.used += 1;
    } else { r.lost = r.lost.saturating_add(1); r.current_row = None; }
    unsafe {
        native_cyclic_return_cell = r.current_row.map_or(core::ptr::null_mut(),
            |i| &raw mut ROWS[i][28]);
    }
    r.compare = target;
    r.sequence = seq;
    r.end = edge;
    r.owner = next;
    next
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    if let Some(i) = state().current_row {
        unsafe { ROWS[i][27] = root(); ROWS[i][28] = counter(); }
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    if let Some(i) = state().current_row { unsafe { ROWS[i][29] = stamp; } }
}

fn transfer(next: usize, f: &mut NativeFrame) {
    if next == TERMINAL { finish(); }
    if next == IDLE_OWNER {
        unsafe { FRAMES[CURRENT] = *f; native_sleep_switch(idle_top()); }
        activate(state().owner, f);
    } else { activate(next, f); }
}

pub(super) fn irq(now: u64, frame: *mut NativeFrame, iar: u32) -> u64 {
    let irq = iar & 0x3ff;
    let f = unsafe { frame.as_mut() };
    let next = select(now, 1, irq as u64, 0, f.as_deref());
    if irq < 1020 {
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
        context::gic_write(context::GICC + 0x10, iar);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
    }
    if let Some(f) = f { transfer(next, f); }
    (next < IDLE_OWNER) as u64
}

pub(super) fn syscall(now: u64, f: &mut NativeFrame) {
    let id = state().owner;
    if id >= IDLE_OWNER || id != unsafe { CURRENT } { fail(21); }
    let op = f.x[8];
    let arg = f.x[0];
    f.x[0] = match op {
        1 => {
            state().exited |= 1 << id;
            unsafe { EXIT_STATUS[id] = arg as i32; }
            head(9, arg);
            arg as i64
        }
        6 => clock_gettime(arg, f.x[1] as usize, now),
        7 => match sleep_until(f) {
            Err(errno) => errno,
            Ok(at) if at.saturating_sub(now) > i64::MAX as u64 => -75,
            Ok(at) => { state().wake[id] = (at > now).then_some(at); 0 }
        },
        8 => 0,
        9..=18 => -95,
        _ => -38,
    } as u64;
    let next = select(now, 2, op, arg, Some(f));
    transfer(next, f);
}

fn print(label: &[u8], values: &[u64]) {
    uart_write(b"NATIVE CYCLIC "); uart_write(label);
    for &n in values { uart_write(b" "); unsigned(n); }
    uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    print(b"CONFIG", &[1, r.epoch, r.hz, r.table.frame_ticks()]);
    for (index, &(owner, ms)) in NATIVE_PARTITION_WINDOWS.iter().enumerate() {
        print(b"WINDOW", &[index as u64, owner.unwrap_or(IDLE_OWNER) as u64, ms, ms * (r.hz / 1000)]);
    }
    for i in 0..r.used { print(b"EVENT", unsafe { &*(&raw const ROWS[i]) }); }
    print(b"DROPPED", &[r.lost]);
    for id in 0..2 { event(b"EXIT", id, Some(i64::from(unsafe { EXIT_STATUS[id] }))); }
    head(10, 1); head(1, 3);
    park()
}
