//! Single-CPU native service plus independently runnable background process.
use super::*;
use crate::aperiodic_policy::{AperiodicServer, ServerKind};

const IDLE_OWNER: usize = 2;
const TERMINAL: usize = 3;
const LIMIT: usize = 2048;
static mut ROWS: [[u64; 37]; LIMIT] = [[0; 37]; LIMIT];
static mut CREDITS: [[(u64, u32); 8]; LIMIT] = [[(0, 0); 8]; LIMIT];
static mut STATE: Option<Runtime> = None;
static mut LAST_RETURN: u64 = 0;
// Reuse the late register-preserving ERET and EL1 idle landing in the native vectors.
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = &raw mut LAST_RETURN;
unsafe extern "C" { static native_cyclic_idle_landing: u8; }

struct Runtime {
    ledger: AperiodicServer<8>,
    kind: u64,
    hz: u64,
    epoch: u64,
    h: u64,
    compare: u64,
    owner: usize,
    grant: bool,
    wake: [Option<u64>; 2],
    exited: u8,
    used: usize,
    lost: u64,
    row: Option<usize>,
    occupied: [u64; 3],
}

fn state() -> &'static mut Runtime { unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(22)) } }
fn idle_top() -> *mut u8 { (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES) }
fn root() -> u64 {
    let value;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack, preserves_flags)); }
    value
}

pub(super) fn start(hz: u64) -> usize {
    let (kind, tag) = match NATIVE_POLICY {
        "polling" => (ServerKind::Polling, 0), "deferrable" => (ServerKind::Deferrable, 1),
        "sporadic" => (ServerKind::Sporadic, 2), _ => fail(22),
    };
    if hz == 0 || hz % 1000 != 0 || COUNT != 2 { fail(22); }
    let h = NATIVE_SERVER_SLOT_MS.checked_mul(hz / 1000)
        .filter(|&v| v != 0 && v <= i64::MAX as u64).unwrap_or_else(|| fail(22));
    let ledger = AperiodicServer::new_live(kind, 0, u32::try_from(NATIVE_SERVER_QUOTA).unwrap_or_else(|_| fail(22)),
        u32::try_from(NATIVE_SERVER_PERIOD).unwrap_or_else(|_| fail(22)), NATIVE_SERVER_CREDITS)
        .unwrap_or_else(|| fail(22));
    let epoch = counter().checked_add(hz / 10).unwrap_or_else(|| fail(22));
    epoch.checked_add(h).unwrap_or_else(|| fail(22));
    unsafe {
        STATE = Some(Runtime { ledger, kind: tag, hz, epoch, h, compare: 0, owner: IDLE_OWNER,
            grant: false, wake: [None; 2], exited: 0, used: 0, lost: 0, row: None, occupied: [0; 3] });
        asm!("msr vbar_el1, {}", "isb", in(reg) &raw const native_el0_vectors, options(nostack));
    }
    let now = counter();
    if now >= epoch { fail(22); }
    select(now, now, 0, 0, 0, None, Action::Keep);
    unsafe { native_sleep_switch(idle_top()); }
    state().owner
}

enum Action { Keep, Exit, Sleep(u64), Yield }

fn select(entry: u64, decision: u64, kind: u64, op: u64, arg: u64,
          f: Option<&NativeFrame>, action: Action) -> usize {
    let r = state();
    let src = r.owner;
    let start = unsafe { ptr::read_volatile(&raw const LAST_RETURN) };
    if let Some(i) = r.row { unsafe { ROWS[i][32] = start; } }
    let occupied = if start == 0 { 0 } else { entry.checked_sub(start).unwrap_or_else(|| fail(22)) };
    r.occupied[src] = r.occupied[src].checked_add(occupied).unwrap_or_else(|| fail(22));
    let cell = decision.checked_sub(r.epoch).map(|v| v / r.h);
    let expected = r.ledger.now();
    let grid = cell.is_some_and(|k| k >= expected);
    let skip = cell.map_or(0, |k| k.saturating_sub(expected));
    let before = r.ledger.remaining_quota();
    if skip != 0 {
        let k = cell.unwrap_or_else(|| fail(22));
        // No skipped cell ran the service. Polls still see the historical
        // demand on either side of its previously registered sleep deadline.
        if let Some(wake) = r.wake[0] {
            let eligible = wake.saturating_sub(r.epoch).div_ceil(r.h);
            let split = eligible.clamp(expected, k);
            r.ledger.skip_live_to(split, false).unwrap_or_else(|_| fail(22));
            r.ledger.skip_live_to(k, r.exited & 1 == 0 && eligible <= k).unwrap_or_else(|_| fail(22));
        } else {
            r.ledger.skip_live_to(k, r.exited & 1 == 0).unwrap_or_else(|_| fail(22));
        }
    }
    let surrender = src == 0 && !matches!(action, Action::Keep);
    match action {
        Action::Exit => r.exited |= 1 << src,
        Action::Sleep(at) => r.wake[src] = Some(at),
        Action::Keep | Action::Yield => {}
    }
    if surrender { r.grant = false; r.ledger.surrender_live(); }
    let wake_service = r.wake[0];
    let mut ready = 0u8;
    let mut woke = 0u8;
    for (id, deadline) in r.wake.iter_mut().enumerate() {
        if deadline.is_some_and(|at| at <= decision) { *deadline = None; woke |= 1 << id; }
        if deadline.is_none() && r.exited & (1 << id) == 0 { ready |= 1 << id; }
    }
    let pre_tick = r.ledger.remaining_quota();
    let at = |k: u64| k.checked_mul(r.h).and_then(|v| r.epoch.checked_add(v)).unwrap_or_else(|| fail(22));
    let mut debit = false;
    if grid {
        let edge = at(expected + skip);
        let demand = ready & 1 != 0 && wake_service.is_none_or(|wake| wake <= edge);
        debit = r.ledger.tick_live(demand, true).unwrap_or_else(|_| fail(22));
        r.grant = debit;
    }
    let next = if r.exited == 3 { TERMINAL } else if r.grant && ready & 1 != 0 { 0 }
        else if cell.is_some() && ready & 2 != 0 { 1 } else { IDLE_OWNER };
    let edge = at(r.ledger.now());
    let compare = r.wake.iter().flatten().copied().fold(edge, u64::min);
    arm_compare((next != TERMINAL).then_some(compare));
    let arm_stamp = counter();
    if r.used < LIMIT {
        let index = r.used;
        let (src_pc, src_sp) = if let Some(f) = f { (f.elr, f.sp_el0) } else if kind == 1 {
            let pc;
            unsafe { asm!("mrs {}, elr_el1", out(reg) pc, options(nostack, preserves_flags)); }
            (pc, idle_top() as u64)
        } else { (0, 0) };
        let dest = if next >= IDLE_OWNER { None } else if next == src && f.is_some() { f }
            else { Some(unsafe { &*(&raw const FRAMES[next]) }) };
        let (pc, sp) = dest.map_or_else(|| if next == IDLE_OWNER {
            (&raw const native_cyclic_idle_landing as u64, idle_top() as u64)
        } else { (0, 0) }, |f| (f.elr, f.sp_el0));
        unsafe {
            ROWS[index] = [index as u64, kind, entry, decision, op, arg, f.map_or(0, |f| f.x[0]),
                src as u64, next as u64, expected, cell.unwrap_or(NO_PID), skip, r.compare,
                if next == TERMINAL { 0 } else { compare }, entry.saturating_sub(at(expected)),
                ready as u64, woke as u64, r.wake[0].unwrap_or(0), r.wake[1].unwrap_or(0), r.exited as u64,
                grid as u64, debit as u64, surrender as u64, before as u64, pre_tick as u64,
                r.ledger.remaining_quota() as u64, src_pc, src_sp, root(), pc, sp, 0, 0, 0, start, occupied, arm_stamp];
            for (j, credit) in r.ledger.pending_replenishments().iter().enumerate() {
                CREDITS[index][j] = (credit.due, credit.amount);
            }
        }
        r.row = Some(index); r.used += 1;
    } else { r.row = None; r.lost = r.lost.saturating_add(1); }
    r.owner = next; r.compare = compare;
    next
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    if let Some(i) = state().row { unsafe { ROWS[i][31] = root(); } }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    if let Some(i) = state().row { unsafe { ROWS[i][33] = stamp; } }
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
    let next = select(now, now, 1, irq as u64, 0, f.as_deref(), Action::Keep);
    if irq < 1020 {
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
        context::gic_write(context::GICC + 0x10, iar);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
    }
    if let Some(f) = f { transfer(next, f); }
    (next < IDLE_OWNER) as u64
}

pub(super) fn syscall(entry: u64, f: &mut NativeFrame) {
    let id = state().owner;
    if id >= IDLE_OWNER || id != unsafe { CURRENT } { fail(22); }
    let op = f.x[8];
    let arg = f.x[0];
    let mut action = Action::Keep;
    f.x[0] = match op {
        1 => { action = Action::Exit; unsafe { EXIT_STATUS[id] = arg as i32; } head(9, arg); arg as i64 }
        6 => clock_gettime(arg, f.x[1] as usize, entry),
        7 => match sleep_until(f) {
            Err(errno) => errno,
            Ok(at) if at.saturating_sub(entry) > i64::MAX as u64 => -75,
            Ok(at) => { if at > entry { action = Action::Sleep(at); } 0 }
        },
        8 => { action = Action::Yield; 0 }
        9..=18 => -95,
        _ => -38,
    } as u64;
    let next = select(entry, counter(), 2, op, arg, Some(f), action);
    transfer(next, f);
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(b"NATIVE SERVER "); uart_write(label);
    for &v in fields { uart_write(b" "); unsigned(v); }
    uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    print(b"CONFIG", &[1, r.kind, r.epoch, r.hz, r.h, NATIVE_SERVER_SLOT_MS, NATIVE_SERVER_QUOTA,
        NATIVE_SERVER_PERIOD, NATIVE_SERVER_CREDITS as u64]);
    for i in 0..r.used {
        print(b"EVENT", unsafe { &*(&raw const ROWS[i]) });
        let credits = unsafe { &*(&raw const CREDITS[i]) };
        let count = credits.iter().take_while(|(_, amount)| *amount != 0).count();
        uart_write(b"NATIVE SERVER CREDITS "); unsigned(i as u64); uart_write(b" "); unsigned(count as u64);
        for &(due, amount) in &credits[..count] { uart_write(b" "); unsigned(due); uart_write(b" "); unsigned(amount as u64); }
        uart_write(b"\n");
    }
    print(b"OCCUPIED", &r.occupied);
    print(b"DROPPED", &[r.lost]);
    for id in 0..2 { event(b"EXIT", id, Some(i64::from(unsafe { EXIT_STATUS[id] }))); }
    head(10, 1); head(1, 3);
    park()
}
