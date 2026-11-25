//! Single CPU native scheduling extension. The helper evaluates private copies;
//! the timer handler owns all commits, context selection and timer programming.
use super::*;
use extension_policy::{Extension, Reservation};
#[path = "native_extension_transaction.rs"]
mod transaction;
use transaction::Transaction;
use group_policy::DelegatedTask;

const IDLE_ID: usize = 3;
const END: usize = 4;
const NONE: u64 = u64::MAX;
const GENERATIONS: [u64; 3] = [7, 11, 17];
const CAP: usize = 512;
const WIDTH: usize = 40;
struct Runtime {
    hz: u64,
    epoch: u64,
    quantum: u64,
    tx: Transaction,
    start: u64,
    end: u64,
    owner: usize,
    compare: u64,
    sleeps: [Option<u64>; 3],
    ready_since: [u64; 3],
    exited: u8,
    row: Option<usize>,
    used: usize,
    dropped: u64,
}
static mut STATE: Option<Runtime> = None;
static mut EVALUATOR: context::Stack = context::Stack([0; context::STACK_BYTES]);
static mut ROWS: [[u64; WIDTH]; CAP] = [[0; WIDTH]; CAP];
#[unsafe(no_mangle)]
static mut native_extension_caller: u64 = 0;
#[unsafe(no_mangle)]
static mut native_server_compare: u64 = NONE;
#[unsafe(no_mangle)]
static mut native_server_last_return: u64 = 0;
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = ptr::null_mut();

fn state() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(26)) }
}
fn idle_top() -> *mut u8 {
    (&raw mut IDLE)
        .cast::<u8>()
        .wrapping_add(context::STACK_BYTES)
}
fn root() -> u64 {
    let value: u64;
    unsafe {
        asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack,preserves_flags));
    }
    value
}
fn tasks(mask: u8) -> [DelegatedTask; 3] {
    core::array::from_fn(|i| DelegatedTask {
        generation: GENERATIONS[i],
        ready: mask & (1 << i) != 0,
        allowed_cpus: 1,
        grant_limit: 1,
    })
}
impl Runtime {
    fn ready(&self, at: u64) -> u8 {
        let mut mask = 0;
        for i in 0..3 {
            if self.exited & (1 << i) == 0
                && self.ready_since[i] <= at
                && self.sleeps[i].is_none_or(|t| t <= at)
            {
                mask |= 1 << i;
            }
        }
        mask
    }
    fn arm(&mut self) {
        let deadline = if self.tx.cell.is_none() {
            self.epoch
        } else if self.tx.evaluating {
            self.tx.cutoff
        } else {
            self.end
        };
        self.compare = self
            .sleeps
            .iter()
            .flatten()
            .copied()
            .min()
            .unwrap_or(deadline)
            .min(deadline);
        arm_compare(Some(self.compare));
        unsafe {
            native_server_compare = if self.owner < 3 { self.end } else { NONE };
        }
    }
}

pub(super) fn start(hz: u64) -> usize {
    let q = hz / 10;
    if COUNT != 3 || q / 4 == 0 || q / 4 >= q / 2 {
        fail(26);
    }
    let epoch = counter().checked_add(q).unwrap_or_else(|| fail(26));
    epoch.checked_add(q).unwrap_or_else(|| fail(26));
    let policy = Extension::new(
        Reservation {
            task: 0,
            period: 3,
            offset: 2,
            ticks: 1,
        },
        0,
        0,
        2,
    )
    .unwrap_or_else(|| fail(26));
    unsafe {
        STATE = Some(Runtime {
            hz,
            epoch,
            quantum: q,
            tx: Transaction::new(policy, tasks(0)),
            start: 0,
            end: 0,
            owner: IDLE_ID,
            compare: 0,
            sleeps: [None; 3],
            ready_since: [epoch; 3],
            exited: 0,
            row: None,
            used: 0,
            dropped: 0,
        });
        asm!("msr vbar_el1, {}", "isb", in(reg) &raw const native_el0_vectors, options(nostack));
    }
    state().arm();
    unsafe {
        native_sleep_switch(idle_top());
    }
    state().owner
}

#[unsafe(no_mangle)]
extern "C" fn native_extension_stack_if_pending() -> *mut u8 {
    let r = state();
    if r.tx.pending() && counter() < r.tx.cutoff {
        (&raw mut EVALUATOR)
            .cast::<u8>()
            .wrapping_add(context::STACK_BYTES)
    } else {
        ptr::null_mut()
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_extension_evaluate() {
    let evaluation = state().tx.begin().unwrap_or_else(|| fail(26));
    unsafe extern "C" {
        fn jnox_extension_evaluator_enter(cell: u64);
    }
    // The entry enables IRQs on this private evaluator stack. A cutoff IRQ
    // may abandon it before the copy reaches evaluation or publication.
    unsafe { jnox_extension_evaluator_enter(evaluation.cell) };
    let reply = evaluation.evaluate(NATIVE_EXTENSION_PROGRAM);
    unsafe {
        asm!("msr daifset,#2", "isb", options(nostack, preserves_flags));
    }
    state().tx.publish(reply, counter);
}

enum Action {
    Keep,
    Surrender,
    Sleep(u64),
    Exit,
}
fn record(r: &mut Runtime, row: [u64; WIDTH]) {
    if r.used < CAP {
        let i = r.used;
        unsafe {
            ROWS[i] = row;
        }
        r.row = Some(i);
        r.used += 1;
    } else {
        r.row = None;
        r.dropped = r.dropped.saturating_add(1);
    }
}
fn settle(
    entry: u64,
    cause: u64,
    op: u64,
    arg: u64,
    frame: Option<&NativeFrame>,
    action: Action,
) -> usize {
    let now = counter();
    let r = state();
    let src = r.owner;
    if frame.is_some() != (src < 3) {
        fail(26);
    }
    let old = r.compare;
    let old_cell = r.tx.cell;
    let old_eval = r.tx.evaluating;
    let (hardware_compare, hardware_control): (u64, u64);
    unsafe {
        asm!("mrs {}, cntp_cval_el0","mrs {}, cntp_ctl_el0",out(reg) hardware_compare,
                 out(reg) hardware_control,options(nostack,preserves_flags));
    }
    let mut skipped = 0;
    let mut published = 0;
    let mut reason = NONE;
    let mut steps = 0;
    let mut grant = 0;
    let mut canceled = false;
    let mut woke = 0;
    let mut source_pc = 0;
    let mut source_sp = 0;
    if let Some(f) = frame {
        source_pc = f.elr;
        source_sp = f.sp_el0;
        if unsafe { CURRENT } != src {
            fail(26);
        }
    } else if cause == 1 {
        unsafe {
            asm!("mrs {}, elr_el1",out(reg) source_pc,options(nostack,preserves_flags));
        }
    }
    match action {
        Action::Exit => {
            r.exited |= 1 << src;
            r.owner = IDLE_ID;
        }
        Action::Sleep(t) => {
            r.ready_since[src] = t;
            r.sleeps[src] = Some(t);
            r.owner = IDLE_ID;
        }
        Action::Surrender => {
            r.owner = IDLE_ID;
        }
        Action::Keep => {}
    }
    // An SVC/restore crossing surrenders the obsolete frame. Only an actual
    // timer acknowledgment opens a new cell or consumes the evaluator reply.
    let physical = cause == 1 && op & 1023 == 30;
    if r.exited == 7 {
        r.owner = END;
        r.tx.abandon();
        r.compare = 0;
        arm_compare(None);
    } else {
        if physical && now >= r.epoch {
            let k = (now - r.epoch) / r.quantum;
            if r.tx.cell != Some(k) {
                skipped = k.saturating_sub(r.tx.cell.map_or(0, |v| v + 1));
                canceled = r.tx.evaluating;
                r.start = now - (now - r.epoch) % r.quantum;
                r.end = r.start.checked_add(r.quantum).unwrap_or_else(|| fail(26));
                let cutoff = r.start + r.quantum / 4;
                let snapshot = tasks(r.ready(r.start));
                r.owner = IDLE_ID;
                if let Some(d) = r.tx.open(k, cutoff, snapshot) {
                    reason = d.reason as u64;
                    if let Some(p) = d.selected {
                        r.owner = p.task;
                        grant = p.ticks;
                    }
                }
            }
            if let Some((d, publication)) = r.tx.consume(k, now) {
                if let Some(stamp) = publication {
                    published = stamp;
                } else {
                    canceled = true;
                }
                reason = d.reason as u64;
                steps = u64::from(d.steps);
                if let Some(p) = d.selected {
                    r.owner = p.task;
                    grant = p.ticks;
                }
            }
        }
        // Preserve opening-time eligibility even when the interrupt arrives late.
        // A wake after S stays represented until the snapshot for S is taken.
        for i in 0..3 {
            if r.sleeps[i].is_some_and(|t| t <= now) {
                r.sleeps[i] = None;
                woke |= 1 << i;
            }
        }
        if !physical && r.tx.cell.is_some() && now >= r.end {
            r.owner = IDLE_ID;
        }
        r.arm();
    }
    let mut dst_pc = 0;
    let mut dst_sp = 0;
    if r.owner < 3 {
        let f = if src == r.owner {
            frame.unwrap()
        } else {
            unsafe { &*(&raw const FRAMES[r.owner]) }
        };
        dst_pc = f.elr;
        dst_sp = f.sp_el0;
    }
    let mask = r.tx
        .snapshot
        .iter()
        .enumerate()
        .fold(0, |v, (i, t)| v | (u64::from(t.ready) << i));
    let row = [
        cause,
        op,
        entry,
        now,
        old_cell.unwrap_or(NONE),
        r.tx.cell.unwrap_or(NONE),
        r.start,
        r.tx.cutoff,
        r.end,
        src as u64,
        r.owner as u64,
        old,
        r.compare,
        skipped,
        u64::from(old_eval),
        u64::from(r.tx.evaluating),
        published,
        u64::from(canceled),
        reason,
        steps,
        r.tx.heartbeat,
        mask,
        grant,
        woke,
        r.exited as u64,
        source_pc,
        source_sp,
        dst_pc,
        dst_sp,
        root(),
        0,
        0,
        0,
        arg,
        r.sleeps[0].unwrap_or(0),
        r.sleeps[1].unwrap_or(0),
        r.sleeps[2].unwrap_or(0),
        hardware_compare,
        hardware_control,
        frame.map_or(0, |f| f.x[0]),
    ];
    record(r, row);
    r.owner
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    if let Some(i) = state().row {
        unsafe {
            ROWS[i][30] = root();
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    if let Some(i) = state().row {
        unsafe {
            ROWS[i][32] = stamp;
        }
    }
}
fn before_event() {
    let stamp = unsafe { native_server_last_return };
    if let Some(i) = state().row {
        unsafe {
            ROWS[i][31] = stamp;
        }
    }
    unsafe {
        native_server_last_return = 0;
    }
}
fn transfer(next: usize, f: &mut NativeFrame) {
    if next == END {
        finish();
    }
    if next == IDLE_ID {
        unsafe {
            FRAMES[CURRENT] = *f;
            native_sleep_switch(idle_top());
        }
        activate(state().owner, f);
    } else {
        activate(next, f);
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_server_reselect(at: u64, frame: *mut NativeFrame) -> u64 {
    before_event();
    let f = unsafe { frame.as_mut() };
    let next = settle(at, 3, 0, 0, f.as_deref(), Action::Surrender);
    if let Some(f) = f {
        transfer(next, f);
    }
    (next < 3) as u64
}
pub(super) fn irq(entry: u64, frame: *mut NativeFrame, iar: u32) -> u64 {
    before_event();
    let f = unsafe { frame.as_mut() };
    let next = settle(entry, 1, u64::from(iar), 0, f.as_deref(), Action::Keep);
    if iar & 1023 < 1020 {
        unsafe {
            asm!("dsb sy", options(nostack, preserves_flags));
        }
        context::gic_write(context::GICC + 0x10, iar);
        unsafe {
            asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
    if let Some(f) = f {
        transfer(next, f);
    }
    (next < 3) as u64
}
pub(super) fn syscall(entry: u64, f: &mut NativeFrame) {
    before_event();
    let id = unsafe { CURRENT };
    if state().owner != id || id >= 3 {
        fail(26);
    }
    let (op, arg) = (f.x[8], f.x[0]);
    let mut action = Action::Keep;
    f.x[0] = match op {
        1 => {
            unsafe {
                EXIT_STATUS[id] = arg as i32;
            }
            action = Action::Exit;
            arg
        }
        6 => clock_gettime(arg, f.x[1] as usize, entry) as u64,
        7 => match sleep_until(f) {
            Err(e) => e as u64,
            Ok(t) if t.saturating_sub(entry) > i64::MAX as u64 => (-75i64) as u64,
            Ok(t) => {
                if t > entry {
                    action = Action::Sleep(t);
                }
                0
            }
        },
        8 => {
            action = Action::Surrender;
            0
        }
        9..=18 => (-95i64) as u64,
        _ => (-38i64) as u64,
    };
    let next = settle(entry, 2, op, arg, Some(f), action);
    transfer(next, f);
}
fn print(tag: &[u8], v: &[u64]) {
    uart_write(b"NATIVE EXTENSION ");
    uart_write(tag);
    for x in v {
        uart_write(b" ");
        unsigned(*x);
    }
    uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    print(b"CONFIG", &[1, r.epoch, r.hz, r.quantum, 5, 2, 3]);
    for i in 0..r.used {
        print(b"INDEX", &[i as u64]);
        print(b"EVENT", unsafe { &*(&raw const ROWS[i]) });
    }
    print(b"DROPPED", &[r.dropped]);
    for i in 0..3 {
        event(b"EXIT", i, Some(i64::from(unsafe { EXIT_STATUS[i] })));
    }
    head(10, 1);
    head(1, 3);
    park()
}
