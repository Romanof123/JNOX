//! Single-process four-role fixed-priority thread adapter. All state transitions
//! occur on CPU0 with IRQ masked; user frames share only their address space.
use super::*;
#[path = "native_priority_threads.rs"]
mod policy;
use policy::{Completion, Policy, State};

static mut POLICY: Option<Policy> = None;
static mut SAVED: [NativeFrame; 4] = [EMPTY_FRAME; 4];
static mut COMPARE: Option<u64> = None;
static mut PENDING: bool = false;
const LIMIT: usize = 1024;
static mut USED: usize = 0;
static mut LOST: u64 = 0;
static mut RETURN_ROW: Option<usize> = None;
static mut EVENTS: [[u64; 25]; LIMIT] = [[0; 25]; LIMIT];
static mut TASKS: [[[u64; 10]; 4]; LIMIT] = [[[0; 10]; 4]; LIMIT];
static mut LOCKS: [[[u64; 10]; 4]; LIMIT] = [[[0; 10]; 4]; LIMIT];

fn policy() -> &'static mut Policy { unsafe { (&mut *(&raw mut POLICY)).as_mut().unwrap_or_else(|| fail(20)) } }
fn bounds(id: usize) -> (usize, usize) { native_el0_mmu::thread_stack(id).unwrap_or_else(|| fail(20)) }
fn locator(id: usize) -> u64 { native_el0_mmu::thread_errno(id).unwrap_or_else(|| fail(20)) as u64 }
fn validate(id: usize, f: &NativeFrame) {
    let (lo, hi) = bounds(id);
    if !(lo as u64..=hi as u64).contains(&f.sp_el0) || f.sp_el0 % 16 != 0 || f.tls[1] != locator(id)
        || f.current_el != 4 || f.spsr & 0x1f != 0 || !native_el0_mmu::valid_thread_entry(f.elr) { fail(20); }
}
fn saved(id: usize) -> &'static NativeFrame { unsafe { &*(&raw const SAVED[id]) } }
fn program(now: u64) {
    let next = policy().next_timer(now);
    if next != unsafe { COMPARE } { arm_compare(next); unsafe { COMPARE = next; } }
}

fn describe(s: State) -> [u64; 3] {
    match s {
        State::Vacant => [0,0,0], State::Running => [1,0,0], State::Ready => [2,0,0],
        State::Sleeping(at) => [3,at,0], State::Joining(handle) => [4,handle,0],
        State::Zombie(value) => [5,value,0], State::Ended => [6,0,0],
        State::MutexWait(token, deadline) => [7,token,deadline.unwrap_or(0)],
    }
}

fn record(entry: u64, now: u64, cause: u64, prior: Option<usize>, original: Option<&NativeFrame>,
          old_compare: Option<u64>) {
    let next = policy().current();
    unsafe { PENDING = next.is_some(); RETURN_ROW = None; }
    let index = unsafe { USED };
    if index == LIMIT { unsafe { LOST = LOST.saturating_add(1); } return; }
    let p = policy();
    let chosen = next.map(saved);
    let ready = p.ready_order().iter().fold(0, |bits, &id| bits << 4 | (id as u64 + 1));
    let op = if cause == 1 { original.map_or(0, |f| f.x[8]) } else { 0 };
    let root;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) root, options(nostack, preserves_flags)); }
    unsafe {
        EVENTS[index] = [index as u64, cause, entry, now, prior.map_or(NO_PID, |i| i as u64),
            next.map_or(NO_PID, |i| i as u64), op, original.map_or(0, |f| f.x[0]),
            original.map_or(0, |f| f.x[1]), original.map_or(0, |f| f.x[2]), original.map_or(0, |f| f.x[3]),
            original.map_or(0, |f| f.x[4]), prior.map_or(0, |id| saved(id).x[0]),
            original.map_or(0, |f| f.elr), original.map_or(0, |f| f.sp_el0), original.map_or(0, |f| f.tls[1]),
            chosen.map_or(0, |f| f.elr), chosen.map_or(0, |f| f.sp_el0), chosen.map_or(0, |f| f.tls[1]),
            chosen.map_or(0, |f| f.x[0]), ready, old_compare.unwrap_or(0), COMPARE.unwrap_or(0), root, 0];
        for id in 0..4 {
            let [tag, value, due] = describe(p.state(id));
            TASKS[index][id] = [p.handle(id), p.base(id) as u64, p.effective(id) as u64, p.is_rr(id) as u64,
                p.residual(id), tag, value, due, saved(id).x[0], saved(id).x[1]];
            let m = &p.mutex_slots()[id];
            let [a,b,c,d] = m.waiters.map(|v| v.unwrap_or(NO_PID));
            LOCKS[index][id] = [m.address.unwrap_or(0),m.generation,m.owner.unwrap_or(NO_PID),
                m.protocol as u64,m.users as u64,m.ceiling as u64,a,b,c,d];
        }
        USED += 1;
        RETURN_ROW = next.map(|_| index);
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_threads_return() {
    if !unsafe { core::mem::replace(&mut *(&raw mut PENDING), false) } { return; }
    let stamp = counter();
    policy().resume(stamp);
    program(stamp);
    unsafe {
        if let Some(i) = (*(&raw mut RETURN_ROW)).take() {
            EVENTS[i][22] = COMPARE.unwrap_or(0); EVENTS[i][24] = stamp;
        }
    }
}

pub(super) fn start(mut main: NativeFrame) -> u64 {
    let now = counter();
    let q = (NATIVE_POLICY == "rr").then_some(NATIVE_QUANTUM_TICKS);
    if NATIVE_THREAD_PRIORITIES.iter().any(|&v| !(1..=31).contains(&v)) ||
        !matches!(NATIVE_POLICY, "fifo" | "rr") || q.is_some_and(|v| v == 0 || v > i64::MAX as u64) { fail(20); }
    unsafe { POLICY = Some(Policy::new(NATIVE_THREAD_PRIORITIES, q)); }
    policy().init_epoch(now);
    main.tls[1] = locator(0);
    validate(0, &main);
    unsafe { SAVED[0] = main; }
    record(now, now, 0, None, None, None);
    main.tls[1]
}

fn create(f: &mut NativeFrame) -> Result<(), i32> {
    let [start, arg, trampoline, mode, priority] = [f.x[0],f.x[1],f.x[2],f.x[3],f.x[4]];
    if !native_el0_mmu::valid_thread_entry(start) || !native_el0_mmu::valid_thread_entry(trampoline) { return Err(22); }
    let explicit = match mode {
        0 => None,
        1 | 2 => Some((mode == 2, u8::try_from(priority).map_err(|_| 22)?)),
        _ => return Err(95),
    };
    let handle = policy().create(explicit)?;
    let id = (handle & 3) as usize;
    let mut worker = EMPTY_FRAME;
    worker.elr = trampoline; worker.sp_el0 = bounds(id).1 as u64; worker.tls[1] = locator(id);
    worker.x[0] = start; worker.x[1] = arg;
    unsafe { native_el0_mmu::clear_worker(id); SAVED[id] = worker; }
    f.x[1] = handle;
    Ok(())
}

fn invoke(f: &mut NativeFrame, now: u64, deadline: Result<u64,i64>) -> Result<(), i32> {
    let [addr,token,a,b,c] = [f.x[0],f.x[1],f.x[2],f.x[3],f.x[4]];
    let op = f.x[8];
    if matches!(op, 13..=17 | 19) &&
        (addr == 0 || addr % 8 != 0 || !native_el0_mmu::user_range(addr as usize, 8, true)) { return Err(22); }
    match op {
        1 => policy().process_exit(addr as i32),
        7 => {
            let at = deadline.map_err(|e| -e as i32)?;
            if at.saturating_sub(now) > i64::MAX as u64 { return Err(75); }
            if at > now { policy().sleep(at); }
        }
        8 => policy().yield_now(),
        10 => return create(f),
        11 => { if let Some(value) = policy().join(addr)? { f.x[1] = value; } }
        12 => policy().thread_exit(addr),
        13 => {
            if token != 0 { return Err(22); }
            let protocol = u8::try_from(a).map_err(|_| 22)?;
            let (users, ceiling) = if a == 0 && b == 0 { (15, 0) } else {
                (u8::try_from(b).map_err(|_| 22)?, u8::try_from(c).map_err(|_| 22)?)
            };
            f.x[1] = policy().mutex_init(addr, protocol, users, ceiling)?;
        }
        14..=17 => policy().mutex(op, addr, token, None)?,
        19 => {
            let at = deadline.map_err(|e| -e as i32)?;
            if at.saturating_sub(now) > i64::MAX as u64 { return Err(75); }
            policy().mutex(15, addr, token, Some(at))?;
        }
        9 | 18 => return Err(95),
        _ => return Err(38),
    }
    Ok(())
}

fn complete() {
    for id in 0..4 {
        if let Some(value) = policy().take_completion(id) {
            let f = unsafe { &mut *(&raw mut SAVED[id]) };
            match value {
                Completion::Join(value) => { f.x[0] = 0; f.x[1] = value; }
                Completion::Mutex(errno) => f.x[0] = (-i64::from(errno)) as u64,
            }
        }
    }
}

pub(super) fn syscall(entry: u64, frame: &mut NativeFrame) {
    let id = policy().current().unwrap_or_else(|| fail(20));
    validate(id, frame);
    let original = *frame;
    let old = unsafe { COMPARE };
    policy().pause(entry);
    let op = frame.x[8];
    let deadline = match op {
        7 => sleep_until(frame),
        19 if frame.x[2] == 1 => monotonic_target(frame.x[3] as usize),
        _ => Err(-22),
    };
    let clock_result = if op == 6 { clock_gettime(frame.x[0], frame.x[1] as usize, entry) } else { 0 };
    let now = counter();
    policy().advance(now);
    frame.x[0] = if op == 6 { clock_result } else {
        invoke(frame, now, deadline).map_or_else(|errno| -i64::from(errno), |_| 0)
    } as u64;
    unsafe { SAVED[id] = *frame; }
    complete();
    policy().dispatch();
    program(now);
    record(entry, now, 1, Some(id), Some(&original), old);
    if let Some(status) = policy().status() { finish(status); }
    if policy().current().is_none() {
        let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
        unsafe { native_sleep_switch(top); }
    }
    let next = policy().current().unwrap_or_else(|| fail(20));
    validate(next, saved(next));
    *frame = *saved(next);
}

pub(super) fn irq(entry: u64, frame: Option<&mut NativeFrame>) -> bool {
    let prior = policy().current();
    let old = unsafe { COMPARE };
    let original = frame.as_deref().copied();
    if let Some(f) = original {
        let id = prior.unwrap_or_else(|| fail(20));
        validate(id, &f);
        unsafe { SAVED[id] = f; }
        policy().pause(entry);
    } else if prior.is_some() { fail(20); }
    let now = counter();
    policy().advance(now);
    complete();
    policy().dispatch();
    program(now);
    let next = policy().current();
    record(entry, now, if prior.is_some() { 2 } else { 3 }, prior, original.as_ref(), old);
    if let Some(f) = frame {
        let chosen = next.unwrap_or_else(|| fail(20));
        validate(chosen, saved(chosen)); *f = *saved(chosen);
    }
    next.is_some()
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(b"NATIVE PRIORITY "); uart_write(label);
    for &field in fields { uart_write(b" "); unsigned(field); } uart_write(b"\n");
}
fn detail(label: &[u8], event: usize, slot: usize, fields: &[u64]) {
    uart_write(b"NATIVE PRIORITY "); uart_write(label);
    uart_write(b" "); unsigned(event as u64); uart_write(b" "); unsigned(slot as u64);
    for &field in fields { uart_write(b" "); unsigned(field); }
    uart_write(b"\n");
}
fn finish(status: i32) -> ! {
    arm_compare(None);
    print(b"CONFIG", &[1, NATIVE_QUANTUM_TICKS, unsafe { CLOCK_DOMAIN.1 }, LIMIT as u64]);
    for id in 0..4 {
        let (lo, hi) = bounds(id);
        print(b"ROLE", &[id as u64, NATIVE_THREAD_PRIORITIES[id] as u64, lo as u64, hi as u64, locator(id)]);
    }
    unsafe {
        for i in 0..USED {
            print(b"EVENT", &*(&raw const EVENTS[i]));
            for id in 0..4 {
                detail(b"TASK", i, id, &*(&raw const TASKS[i][id]));
                detail(b"LOCK", i, id, &*(&raw const LOCKS[i][id]));
            }
        }
        print(b"DROPPED", &[LOST]);
    }
    head(9,status as i64 as u64); head(10,1); head(1,3);
    uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
    park()
}
