//! Two cooperative EL0 threads in a single process/address space. Handlers
//! run masked on CPU0; the idle timer can run only inside native_sleep_switch.
use super::*;
#[path = "native_thread_policy.rs"]
mod policy;
use policy::{Policy, State};

static mut POLICY: Policy = Policy::new();
static mut SAVED: [NativeFrame; 2] = [EMPTY_FRAME; 2];
static mut COMPARE: Option<u64> = None;
const LIMIT: usize = 512;
static mut ROWS: [[u64; 28]; LIMIT] = [[0; 28]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;
static mut RETURN_ROW: Option<usize> = None;

fn policy() -> &'static mut Policy { unsafe { &mut *(&raw mut POLICY) } }
fn bounds(id: usize) -> (usize, usize) { native_el0_mmu::thread_stack(id).unwrap_or_else(|| fail(20)) }
fn locator(id: usize) -> u64 { native_el0_mmu::thread_errno(id).unwrap_or_else(|| fail(20)) as u64 }

fn validate(id: usize, f: &NativeFrame) {
    let (low, high) = bounds(id);
    if !(low as u64..=high as u64).contains(&f.sp_el0) || f.sp_el0 % 16 != 0
        || f.tls[1] != locator(id) || f.current_el != 4 || f.spsr & 0x1f != 0
        || !native_el0_mmu::valid_thread_entry(f.elr)
    { fail(20) }
}

fn update_compare() {
    let next = policy().next_compare();
    if next != unsafe { COMPARE } {
        arm_compare(next);
        unsafe { COMPARE = next };
    }
}

fn describe(state: State) -> (u64, u64) {
    match state {
        State::Vacant => (0, 0), State::Running => (1, 0), State::Ready => (2, 0),
        State::Sleeping(at) => (3, at), State::Joining => (4, 0),
        State::Zombie(value) => (5, value), State::Ended => (6, 0),
    }
}

fn record(now: u64, cause: u64, prior: Option<usize>, op: u64, args: [u64; 3],
          outgoing: Option<&NativeFrame>, selected: Option<&NativeFrame>, old_compare: Option<u64>) {
    let index = unsafe { USED };
    if index == LIMIT {
        unsafe { LOST = LOST.saturating_add(1); RETURN_ROW = None; }
        return;
    }
    let p = policy();
    let main = describe(p.state(0));
    let worker = describe(p.state(1));
    let ready = p.ready().iter().fold(0, |v, &tid| (v << 4) | (tid as u64 + 1));
    let root: u64;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) root, options(nostack, preserves_flags)); }
    let row = [
        now, cause, prior.map_or(NO_PID, |id| id as u64), p.current().map_or(NO_PID, |id| id as u64),
        op, args[0], args[1], args[2],
        outgoing.map_or(0, |f| f.x[0]), outgoing.map_or(0, |f| f.x[1]),
        outgoing.map_or(0, |f| f.elr), outgoing.map_or(0, |f| f.sp_el0), outgoing.map_or(0, |f| f.tls[1]),
        selected.map_or(0, |f| f.elr), selected.map_or(0, |f| f.sp_el0), selected.map_or(0, |f| f.tls[1]),
        selected.map_or(0, |f| f.x[0]), selected.map_or(0, |f| f.x[1]), p.generation(),
        main.0, worker.0, main.1, worker.1, ready, old_compare.unwrap_or(0),
        unsafe { COMPARE }.unwrap_or(0), root, 0,
    ];
    unsafe {
        ROWS[index] = row;
        USED += 1;
        RETURN_ROW = selected.map(|_| index);
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_threads_return() {
    unsafe {
        if let Some(index) = (*(&raw mut RETURN_ROW)).take() { ROWS[index][27] = counter(); }
    }
}

pub(super) fn start(mut main: NativeFrame) -> u64 {
    main.tls[1] = locator(0);
    validate(0, &main);
    unsafe { SAVED[0] = main; }
    record(counter(), 0, None, 0, [0; 3], None, Some(&main), None);
    main.tls[1]
}

// The kernel constructs both the PC and stack from checked process-owned
// mappings. The opaque argument belongs to user code and is never dereferenced.
fn create(start: u64, arg: u64, trampoline: u64) -> Result<u64, i32> {
    if !native_el0_mmu::valid_thread_entry(start) || !native_el0_mmu::valid_thread_entry(trampoline) {
        return Err(22);
    }
    let handle = policy().create()?;
    let mut worker = EMPTY_FRAME;
    worker.elr = trampoline;
    worker.sp_el0 = bounds(1).1 as u64;
    worker.x[0] = start;
    worker.x[1] = arg;
    worker.tls[1] = locator(1);
    unsafe {
        native_el0_mmu::reset_worker_memory();
        SAVED[1] = worker;
    }
    Ok(handle)
}

fn saved(id: usize) -> &'static NativeFrame { unsafe { &*(&raw const SAVED[id]) } }

pub(super) fn irq(now: u64, frame: Option<&NativeFrame>) -> bool {
    let prior = policy().current();
    let old_compare = unsafe { COMPARE };
    if let Some(f) = frame { validate(prior.unwrap_or_else(|| fail(20)), f); }
    policy().wake_due(now);
    update_compare();
    let next = policy().current();
    // A timer during user execution only enqueues sleepers. An idle IRQ
    // instead selects a saved thread for the suspended kernel continuation.
    if frame.is_some() && prior != next { fail(20) }
    let selected = frame.or_else(|| next.map(saved));
    record(now, if frame.is_some() { 2 } else { 3 }, prior, 0, [0; 3], frame, selected, old_compare);
    next.is_some()
}

pub(super) fn syscall(now: u64, f: &mut NativeFrame) {
    let id = policy().current().unwrap_or_else(|| fail(20));
    validate(id, f);
    let old_compare = unsafe { COMPARE };
    let [a, b, c] = [f.x[0], f.x[1], f.x[2]];
    let op = f.x[8];
    policy().wake_due(now);
    match op {
        1 => policy().process_exit(a as i32),
        6 => f.x[0] = clock_gettime(a, b as usize, now) as u64,
        7 => {
            f.x[0] = match sleep_until(f) {
                Ok(at) => policy().sleep(now, at).map_or_else(|e| -i64::from(e), |_| 0),
                Err(error) => error,
            } as u64;
        }
        8 => { f.x[0] = 0; policy().yield_now(); }
        9 => f.x[0] = (-95i64) as u64,
        10 => match create(a, b, c) {
            Ok(handle) => { f.x[0] = 0; f.x[1] = handle; }
            Err(error) => f.x[0] = (-i64::from(error)) as u64,
        },
        11 => match policy().join(a) {
            Ok(value) => { f.x[0] = 0; if let Some(value) = value { f.x[1] = value; } }
            Err(error) => f.x[0] = (-i64::from(error)) as u64,
        },
        12 => {
            if let Some(value) = policy().thread_exit(a) {
                unsafe { SAVED[0].x[0] = 0; SAVED[0].x[1] = value; }
            }
        }
        _ => f.x[0] = (-38i64) as u64,
    }
    unsafe { SAVED[id] = *f };
    update_compare();
    record(now, 1, Some(id), op, [a, b, c], Some(f), policy().current().map(saved), old_compare);
    if let Some(status) = policy().status() { finish(status); }
    if policy().current().is_none() {
        if policy().next_compare().is_none() { fail(20) }
        let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
        unsafe { native_sleep_switch(top); }
    }
    let next = policy().current().unwrap_or_else(|| fail(20));
    let chosen = saved(next);
    validate(next, chosen);
    *f = *chosen;
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(label);
    for &value in fields { uart_write(b" "); unsigned(value); }
    uart_write(b"\n");
}

fn finish(status: i32) -> ! {
    arm_compare(None);
    head(9, status as i64 as u64); head(10, 1); head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    for id in 0..2 {
        let (low, high) = bounds(id);
        print(b"NATIVE THREAD STACK", &[id as u64, low as u64, high as u64, locator(id)]);
    }
    unsafe {
        for i in 0..USED { print(b"NATIVE THREAD EVENT", &*(&raw const ROWS[i])); }
        print(b"NATIVE THREAD DROPPED", &[LOST]);
    }
    uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
    park()
}
