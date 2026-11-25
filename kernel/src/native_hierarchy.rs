//! Fixed-depth CPU budget tree for independently loaded native applications.
use super::*;
#[path = "hierarchy_policy.rs"]
mod policy;
#[path = "native_hierarchy_trace.rs"]
mod trace;

const P: usize = NATIVE_HIERARCHY_PARENTS.len();
const C: usize = NATIVE_HIERARCHY_CHILDREN.len();
const L: usize = NATIVE_HIERARCHY_LEAVES.len();
const IDLE_OWNER: usize = 3;
const TERMINAL: usize = 4;
type Ledger = policy::Hierarchy<P, C, L>;
unsafe extern "C" { static native_cyclic_idle_landing: u8; }

struct Runtime {
    ledger: Ledger,
    epoch: u64,
    hz: u64,
    owner: usize,
    grant_start: u64,
    grant_end: u64,
    compare: Option<u64>,
    wake: [Option<u64>; L],
    exited: u64,
}
static mut STATE: Option<Runtime> = None;

fn state() -> &'static mut Runtime {
    unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(23)) }
}
fn idle_top() -> *mut u8 { (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES) }
fn root() -> u64 {
    let value;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack, preserves_flags)); }
    value
}

pub(super) fn start(hz: u64) -> usize {
    if !(1..=2).contains(&P) || !(1..=3).contains(&C) || !(1..=3).contains(&L) || COUNT != L {
        fail(23);
    }
    let epoch = counter();
    let ledger = Ledger::new(epoch, NATIVE_HIERARCHY_PERIOD, NATIVE_HIERARCHY_PARENTS,
        NATIVE_HIERARCHY_CHILDREN, NATIVE_HIERARCHY_LEAVES).unwrap_or_else(|| fail(23));
    unsafe {
        STATE = Some(Runtime { ledger, epoch, hz, owner: IDLE_OWNER, grant_start: 0, grant_end: 0,
            compare: None, wake: [None; L], exited: 0 });
    }
    select(epoch, 0, 0, 0, None, Action::Keep)
}

enum Action { Keep, Sleep(u64), Exit }

fn select(entry: u64, kind: u64, op: u64, argument: u64, frame: Option<&NativeFrame>, action: Action) -> usize {
    let r = state();
    let src = r.owner;
    let mut row = trace::Row {
        kind, op, entry, source: src as u64, argument, old_compare: r.compare.unwrap_or(0),
        grant_start: r.grant_start, grant_end: r.grant_end, src_root: root(), ..Default::default()
    };
    if let Some(f) = frame {
        row.src_pc = f.elr; row.src_sp = f.sp_el0; row.result = f.x[0];
        row.src_frame = unsafe { (&raw const FRAMES[src]).addr() as u64 };
    } else if kind == 1 {
        unsafe { asm!("mrs {}, elr_el1", out(reg) row.src_pc, options(nostack, preserves_flags)); }
        row.src_sp = idle_top() as u64;
    }
    // One boundary timestamp both closes the old reservation and opens its
    // successor. Instrumentation never shortens, splits or duplicates this charge.
    let now = counter();
    row.settle_begin = now;
    if src < L {
        let (elapsed, overrun, debts) = r.ledger.finish_at(now).unwrap_or_else(|| fail(23));
        row.elapsed = elapsed; row.overrun = overrun; row.debts = debts;
    }
    row.settle_end = counter();
    match action {
        Action::Keep => {}
        Action::Sleep(target) => r.wake[src] = Some(target),
        Action::Exit => r.exited |= 1 << src,
    }
    let mut ready = [false; L];
    for id in 0..L {
        if r.wake[id].is_some_and(|wake| wake <= now) {
            r.wake[id] = None; row.woke |= 1 << id;
        }
        ready[id] = r.exited & (1 << id) == 0 && r.wake[id].is_none();
        row.ready |= u64::from(ready[id]) << id;
        row.sleepers[id] = r.wake[id].unwrap_or(0);
        let child = NATIVE_HIERARCHY_LEAVES[id].0;
        let parent = NATIVE_HIERARCHY_CHILDREN[child].0;
        let (p, c, l) = r.ledger.remaining_at(now, parent, child, id).unwrap_or_else(|| fail(23));
        row.balances[id] = [p, c, l];
    }
    row.exited = r.exited;
    row.select_begin = counter();
    let grant = r.ledger.grant_at(now, u64::MAX, &ready);
    let (target, policy_due) = if let Some((_, _, id, reserved)) = grant {
        r.grant_start = now;
        r.grant_end = now.checked_add(reserved).unwrap_or_else(|| fail(23));
        row.reserved = reserved;
        (id, Some(r.grant_end))
    } else {
        r.grant_start = 0; r.grant_end = 0;
        let terminal = r.exited == (1u64 << L) - 1;
        (if terminal { TERMINAL } else { IDLE_OWNER }, r.ledger.next_actionable(now, &ready))
    };
    row.select_end = counter();
    r.compare = policy_due.into_iter().chain(r.wake.iter().flatten().copied()).min()
        // CVAL compares using signed counter distances. Revisit a very distant
        // wake/refill in range, retaining the original absolute deadline.
        .map(|due| due.min(now.saturating_add(i64::MAX as u64)));
    if target == IDLE_OWNER && r.compare.is_none() { fail(23); }
    arm_compare(r.compare);
    row.target = target as u64;
    row.next_compare = r.compare.unwrap_or(0);
    if target < L {
        let selected = if src == target && frame.is_some() { frame.unwrap() } else {
            unsafe { &*(&raw const FRAMES[target]) }
        };
        row.dst_pc = selected.elr;
        row.dst_sp = selected.sp_el0;
        row.dst_frame = unsafe { (&raw const FRAMES[target]).addr() as u64 };
    } else if target == IDLE_OWNER {
        row.dst_pc = &raw const native_cyclic_idle_landing as u64;
        row.dst_sp = idle_top() as u64;
    }
    r.owner = target;
    trace::record(row);
    target
}

fn transfer(target: usize, frame: &mut NativeFrame) {
    if target == TERMINAL { finish(); }
    if target == IDLE_OWNER {
        unsafe { FRAMES[CURRENT] = *frame; native_sleep_switch(idle_top()); }
        activate(state().owner, frame);
    } else { activate(target, frame); }
}

pub(super) fn irq(entry: u64, ptr: *mut NativeFrame, iar: u32) -> u64 {
    let id = iar & 0x3ff;
    let f = unsafe { ptr.as_mut() };
    if f.is_some() != (state().owner < L) { fail(23); }
    let next = select(entry, 1, id as u64, 0, f.as_deref(), Action::Keep);
    if id < 1020 {
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
        context::gic_write(context::GICC + 0x10, iar);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
    }
    if let Some(f) = f { transfer(next, f); }
    (next < L) as u64
}

pub(super) fn syscall(entry: u64, frame: &mut NativeFrame) {
    let id = state().owner;
    if id >= L || id != unsafe { CURRENT } { fail(23); }
    let op = frame.x[8];
    let arg = frame.x[0];
    let mut action = Action::Keep;
    frame.x[0] = match op {
        1 => { action = Action::Exit; unsafe { EXIT_STATUS[id] = arg as i32; } head(9, arg); arg as i64 }
        6 => clock_gettime(arg, frame.x[1] as usize, entry),
        7 => match sleep_until(frame) {
            Err(errno) => errno,
            Ok(at) if at.saturating_sub(entry) > i64::MAX as u64 => -75,
            Ok(at) => { if at > entry { action = Action::Sleep(at); } 0 }
        },
        8 => 0,
        9..=18 => -95,
        _ => -38,
    } as u64;
    let target = select(entry, 2, op, arg, Some(frame), action);
    transfer(target, frame);
}

fn print(label: &[u8], values: &[u64]) {
    uart_write(b"NATIVE HIERARCHY "); uart_write(label);
    for &v in values { uart_write(b" "); unsigned(v); } uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    print(b"CONFIG", &[1, r.epoch, r.hz, NATIVE_HIERARCHY_PERIOD, P as u64, C as u64, L as u64]);
    for (id, quota) in NATIVE_HIERARCHY_PARENTS.into_iter().enumerate() { print(b"PARENT", &[id as u64, quota]); }
    for (id, (group, quota)) in NATIVE_HIERARCHY_CHILDREN.into_iter().enumerate() { print(b"CHILD", &[id as u64, group as u64, quota]); }
    for (id, (child, quota)) in NATIVE_HIERARCHY_LEAVES.into_iter().enumerate() { print(b"LEAF", &[id as u64, child as u64, quota]); }
    trace::dump();
    for id in 0..L {
        let status = i64::from(unsafe { EXIT_STATUS[id] });
        if L == 1 { uart_write(b"NATIVE EL0 EXIT "); decimal(status); uart_write(b"\n"); }
        else { event(b"EXIT", id, Some(status)); }
    }
    head(10, 1); head(1, 3);
    park()
}
