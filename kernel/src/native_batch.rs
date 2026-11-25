//! Finite private native jobs. Ready aging follows absolute event time; service
//! is measured from a real final return stamp to the next exception entry.
use super::*;
use crate::batch_policy::{BatchPolicy, NativeBatchScheduler};

const IDLE_OWNER: usize = 3;
const TERMINAL: usize = 4;
const LIMIT: usize = 1024;
const ALL: u64 = (1 << COUNT) - 1;
static mut EVENTS: [[u64; 39]; LIMIT] = [[0; 39]; LIMIT];
static mut DETAILS: [[[u64; 4]; COUNT]; LIMIT] = [[[0; 4]; COUNT]; LIMIT];
static mut STATE: Option<Runtime> = None;
static mut LAST_RETURN: u64 = 0;
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = &raw mut LAST_RETURN;
#[unsafe(no_mangle)]
static mut native_batch_next_compare: u64 = 0;
unsafe extern "C" {
    static native_cyclic_idle_landing: u8;
}

struct Runtime {
    p: NativeBatchScheduler<COUNT>,
    epoch: u64,
    hz: u64,
    arrivals: [u64; COUNT],
    released: u64,
    exited: u64,
    sleepers: [Option<u64>; COUNT],
    compare: Option<u64>,
    owner: usize,
    mode: u64,
    used: usize,
    lost: u64,
    pending: Option<usize>,
    transparent: u64,
    transparent_ticks: u64,
}
fn state() -> &'static mut Runtime {
    unsafe {
        (&mut *(&raw mut STATE))
            .as_mut()
            .unwrap_or_else(|| fail(25))
    }
}
fn idle_top() -> *mut u8 {
    (&raw mut IDLE)
        .cast::<u8>()
        .wrapping_add(context::STACK_BYTES)
}
fn registers() -> (u64, u64, u64) {
    let (cval, ctl, root);
    unsafe {
        asm!("mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0","mrs {},ttbr0_el1",
        out(reg)cval,out(reg)ctl,out(reg)root,options(nostack,preserves_flags));
    }
    (cval, ctl, root)
}

pub(super) fn start(hz: u64) -> usize {
    let (policy, mode) = match NATIVE_POLICY {
        "sjf" => (BatchPolicy::Sjf, 0),
        "srtf" => (BatchPolicy::Srtf, 1),
        "hrrn" => (BatchPolicy::Hrrn, 2),
        _ => fail(25),
    };
    if !(1..=3).contains(&COUNT) || hz == 0 {
        fail(25);
    }
    let epoch = counter()
        .checked_add((hz / 10).max(1))
        .unwrap_or_else(|| fail(25));
    let arrivals = NATIVE_BATCH_JOBS.map(|(offset, estimate)| {
        if offset > i64::MAX as u64 || !(1..=i64::MAX as u64).contains(&estimate) {
            fail(25);
        }
        epoch.checked_add(offset).unwrap_or_else(|| fail(25))
    });
    unsafe {
        STATE = Some(Runtime {
            p: NativeBatchScheduler::new(policy, epoch),
            epoch,
            hz,
            arrivals,
            released: 0,
            exited: 0,
            sleepers: [None; COUNT],
            compare: None,
            owner: IDLE_OWNER,
            mode,
            used: 0,
            lost: 0,
            pending: None,
            transparent: 0,
            transparent_ticks: 0,
        });
        asm!("msr vbar_el1, {}","isb",in(reg)&raw const native_el0_vectors,options(nostack));
    }
    let now = counter();
    if now >= epoch {
        fail(25);
    }
    step(now, now, 0, 0, 0, 0, None, Action::Keep);
    unsafe {
        native_sleep_switch(idle_top());
    }
    state().owner
}

#[derive(Clone, Copy)]
enum Action {
    Keep,
    Yield,
    Sleep(u64),
    Exit,
}
fn next_event(r: &Runtime) -> Option<(u64, bool, usize)> {
    let arrivals = (0..COUNT)
        .filter(|&id| r.released & (1 << id) == 0)
        .map(|id| (r.arrivals[id], false, id));
    let wakes = r
        .sleepers
        .iter()
        .enumerate()
        .filter_map(|(id, &v)| v.map(|at| (at, true, id)));
    arrivals.chain(wakes).min()
}
fn step(
    entry: u64,
    now: u64,
    kind: u64,
    op: u64,
    arg: u64,
    result: u64,
    frame: Option<&NativeFrame>,
    action: Action,
) -> usize {
    let r = state();
    let src = r.owner;
    let (cval, ctl, root) = registers();
    let old = r.compare;
    let recheck = kind == 3;
    let start = if kind != 0 && !recheck {
        unsafe { ptr::read_volatile(&raw const LAST_RETURN) }
    } else {
        0
    };
    if let Some(i) = r.pending.take() {
        if !recheck {
            unsafe {
                EVENTS[i][33] = start;
            }
        }
    }
    let dt = if src < COUNT && !recheck {
        r.p.charge(src, start, entry).unwrap_or_else(|_| fail(25));
        entry.checked_sub(start).unwrap_or_else(|| fail(25))
    } else {
        0
    };
    let due = next_event(r).is_some_and(|(at, _, _)| at <= now);
    // Keep genuinely nonblocking calls out of the bounded scheduling journal.
    // Their actual EL0 intervals remain individually charged and summed.
    if kind == 2 && matches!(action, Action::Keep) && !due {
        r.transparent = r.transparent.checked_add(1).unwrap_or_else(|| fail(25));
        r.transparent_ticks = r
            .transparent_ticks
            .checked_add(dt)
            .unwrap_or_else(|| fail(25));
        return src;
    }
    let mut releases = 0;
    let mut wakes = 0;
    let mut order = 0;
    let sleep = if let Action::Sleep(at) = action {
        at
    } else {
        0
    };
    if let Action::Sleep(at) = action {
        // The target can mature while EL1 is reading the request.
        r.p.suspend(entry, src).unwrap_or_else(|_| fail(25));
        r.sleepers[src] = Some(at);
    }
    while let Some((at, wake, id)) = next_event(r).filter(|(t, _, _)| *t <= now) {
        if wake {
            r.p.wake_due(now, at, id).unwrap_or_else(|_| fail(25));
            r.sleepers[id] = None;
            wakes |= 1 << id;
        } else {
            r.p.release_due(now, at, id, NATIVE_BATCH_JOBS[id].1)
                .unwrap_or_else(|_| fail(25));
            r.released |= 1 << id;
            releases |= 1 << id;
        }
        order = (order << 4) | (id as u64 + if wake { 5 } else { 1 });
    }
    match action {
        Action::Exit => {
            r.p.complete(now, src).unwrap_or_else(|_| fail(25));
            r.exited |= 1 << src;
        }
        Action::Yield => r.p.yield_running(now, src).unwrap_or_else(|_| fail(25)),
        _ => {}
    }
    let arbitration = kind != 0 && (due || !matches!(action, Action::Keep));
    let next = if r.exited == ALL {
        TERMINAL
    } else if arbitration {
        r.p.decide(now)
            .unwrap_or_else(|_| fail(25))
            .unwrap_or(IDLE_OWNER)
    } else {
        src
    };
    let earliest = next_event(r).map(|(at, _, _)| at);
    // A maximum offset is relative to E; bridge a longer signed compare
    // distance once before E without making a job ready.
    r.compare = earliest.map(|at| at.min(now.saturating_add(i64::MAX as u64)));
    if r.compare != old || kind == 0 {
        arm_compare(r.compare);
    }
    unsafe {
        native_batch_next_compare = r.compare.unwrap_or(0);
    }
    let src_regs = if let Some(f) = frame {
        (f.elr, f.sp_el0, f.spsr)
    } else if kind == 1 {
        let (pc, ps);
        unsafe {
            asm!("mrs {},elr_el1","mrs {},spsr_el1",out(reg)pc,out(reg)ps,options(nostack,preserves_flags));
        }
        (pc, idle_top() as u64, ps)
    } else {
        (0, 0, 0)
    };
    let dst = if next >= COUNT {
        None
    } else if src == next && frame.is_some() {
        frame
    } else {
        Some(unsafe { &*(&raw const FRAMES[next]) })
    };
    let dest = dst.map_or_else(
        || {
            if next == IDLE_OWNER {
                (
                    &raw const native_cyclic_idle_landing as u64,
                    idle_top() as u64,
                    5,
                )
            } else {
                (0, 0, 0)
            }
        },
        |f| (f.elr, f.sp_el0, f.spsr),
    );
    let sleeping = r
        .sleepers
        .iter()
        .enumerate()
        .fold(0, |bits, (id, s)| bits | ((s.is_some() as u64) << id));
    let ready = r.released & !r.exited & !sleeping;
    let queue =
        r.p.ready()
            .iter()
            .fold(0, |bits, &id| (bits << 4) | (id as u64 + 1));
    if r.used < LIMIT {
        let i = r.used;
        unsafe {
            EVENTS[i] = [
                i as u64,
                kind,
                entry,
                now,
                op,
                arg,
                result,
                src as u64,
                next as u64,
                releases,
                wakes,
                order,
                ready,
                r.released,
                sleeping,
                r.exited,
                queue,
                old.unwrap_or(0),
                r.compare.unwrap_or(0),
                cval,
                ctl,
                src_regs.0,
                src_regs.1,
                src_regs.2,
                root,
                dest.0,
                dest.1,
                dest.2,
                if next < COUNT {
                    native_el0_mmu::space_id(next).unwrap_or(0)
                } else {
                    root
                },
                if src < COUNT {
                    (&raw const FRAMES[src]).addr() as u64
                } else {
                    0
                },
                if next < COUNT {
                    (&raw const FRAMES[next]).addr() as u64
                } else {
                    0
                },
                start,
                dt,
                0,
                0,
                0,
                sleep,
                r.transparent,
                r.transparent_ticks,
            ];
            for id in 0..COUNT {
                DETAILS[i][id] = [
                    r.p.remaining(id).unwrap_or(NO_PID),
                    r.p.service(id).unwrap_or(NO_PID),
                    r.p.eligible_wait(now, id).unwrap_or(NO_PID),
                    r.sleepers[id].unwrap_or(0),
                ];
            }
        }
        r.used += 1;
        r.pending = Some(i);
    } else {
        r.lost = r.lost.saturating_add(1);
        r.pending = None;
    }
    r.transparent = 0;
    r.transparent_ticks = 0;
    r.owner = next;
    next
}

#[unsafe(no_mangle)]
extern "C" fn native_batch_return() {
    if let Some(i) = state().pending {
        unsafe {
            EVENTS[i][28] = registers().2;
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    native_batch_return();
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    if let Some(i) = state().pending {
        unsafe {
            EVENTS[i][34] = stamp;
        }
    }
}
fn transfer(next: usize, f: &mut NativeFrame) {
    if next == TERMINAL {
        finish();
    }
    if next == IDLE_OWNER {
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
extern "C" fn native_batch_recheck(entry: u64, f: &mut NativeFrame) {
    let next = step(entry, counter(), 3, 0, 0, 0, Some(f), Action::Keep);
    transfer(next, f);
}
pub(super) fn irq(entry: u64, ptr: *mut NativeFrame, iar: u32) -> u64 {
    let f = unsafe { ptr.as_mut() };
    if (iar & 0x3ff) != context::TIMER_IRQ || f.is_some() != (state().owner < COUNT) {
        fail(25);
    }
    let next = step(
        entry,
        counter(),
        1,
        iar as u64,
        0,
        0,
        f.as_deref(),
        Action::Keep,
    );
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    context::gic_write(context::GICC + 0x10, iar);
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    if let Some(i) = state().pending {
        unsafe {
            EVENTS[i][35] = iar as u64;
        }
    }
    if let Some(f) = f {
        transfer(next, f);
    }
    (next < COUNT) as u64
}
pub(super) fn syscall(entry: u64, f: &mut NativeFrame) {
    let id = state().owner;
    if id >= COUNT || id != unsafe { CURRENT } {
        fail(25);
    }
    let op = f.x[8];
    let arg = f.x[0];
    let mut action = Action::Keep;
    let result = match op {
        1 => {
            action = Action::Exit;
            unsafe {
                EXIT_STATUS[id] = arg as i32;
            }
            head(9, arg);
            arg as i64
        }
        6 => clock_gettime(arg, f.x[1] as usize, entry),
        7 => match sleep_until(f) {
            Err(e) => e,
            Ok(at) if at.saturating_sub(entry) > i64::MAX as u64 => -75,
            Ok(at) => {
                if at > entry {
                    action = Action::Sleep(at);
                }
                0
            }
        },
        8 => {
            action = Action::Yield;
            0
        }
        9..=19 => -95,
        _ => -38,
    };
    f.x[0] = result as u64;
    let next = step(entry, counter(), 2, op, arg, result as u64, Some(f), action);
    transfer(next, f);
}
fn print(label: &[u8], v: &[u64]) {
    uart_write(b"NATIVE BATCH ");
    uart_write(label);
    for &n in v {
        uart_write(b" ");
        unsigned(n);
    }
    uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    print(b"CONFIG", &[1, r.mode, r.epoch, r.hz, COUNT as u64]);
    for id in 0..COUNT {
        let (offset, estimate) = NATIVE_BATCH_JOBS[id];
        print(
            b"JOB",
            &[
                id as u64,
                offset,
                estimate,
                native_el0_mmu::space_id(id).unwrap_or(0),
            ],
        );
    }
    unsafe {
        for i in 0..r.used {
            print(b"EVENT", &*(&raw const EVENTS[i]));
            for id in 0..COUNT {
                let [rem, svc, wait, wake] = DETAILS[i][id];
                print(b"STATE", &[i as u64, id as u64, rem, svc, wait, wake]);
            }
        }
    }
    print(b"DROPPED", &[r.lost]);
    for id in 0..COUNT {
        let status = i64::from(unsafe { EXIT_STATUS[id] });
        if COUNT == 1 {
            uart_write(b"NATIVE EL0 EXIT ");
            decimal(status);
            uart_write(b"\n");
        } else {
            event(b"EXIT", id, Some(status));
        }
    }
    head(10, 1);
    head(1, 3);
    park()
}
