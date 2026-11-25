//! Two EL0 threads in a single process/address space. Handlers
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

#[cfg(jnox_native_thread_rr)]
mod rr {
    use super::*;
    use policy::RoundRobin;

    static mut CLOCK: Option<RoundRobin> = None;
    static mut START: Option<u64> = None;
    static mut RETURN_PENDING: bool = false;
    static mut EVENTS: [[u64; 12]; LIMIT] = [[0; 12]; LIMIT];

    fn clock() -> &'static mut RoundRobin {
        unsafe { (&mut *(&raw mut CLOCK)).as_mut().unwrap_or_else(|| fail(20)) }
    }
    pub(super) fn init() {
        unsafe { CLOCK = Some(RoundRobin::new(NATIVE_QUANTUM_TICKS).unwrap_or_else(|_| fail(20))); }
    }
    pub(super) fn compare(now: u64) -> Option<u64> { clock().compare(policy(), now) }

    pub(super) fn enter(now: u64, owner: Option<usize>) {
        let end = clock().logical_end();
        let queue = policy().ready().iter().fold(0, |v, &id| (v << 4) | (id as u64 + 1));
        let start = unsafe { (*(&raw mut START)).take() };
        let elapsed = owner.map_or(0, |id| clock().pause(id, now).unwrap_or_else(|_| fail(20)));
        let i = unsafe { USED };
        if i < LIMIT {
            unsafe { EVENTS[i] = [i as u64, queue, end.unwrap_or(0), start.unwrap_or(0), now,
                elapsed, end.map_or(0, |at| now.saturating_sub(at)), 0, 0, 0,
                clock().service(0), clock().service(1)]; }
        }
    }
    pub(super) fn timer(now: u64) {
        let rotated = clock().timer(policy(), now);
        let i = unsafe { USED };
        if i < LIMIT { unsafe { EVENTS[i][7] = rotated as u64; } }
    }
    pub(super) fn end_turn() { clock().end_turn(); }
    pub(super) fn selected(selected: bool) {
        unsafe { RETURN_PENDING = selected; }
        let i = unsafe { USED };
        if i < LIMIT {
            unsafe { EVENTS[i][8] = clock().logical_end().unwrap_or(0); EVENTS[i][9] = COMPARE.unwrap_or(0); }
        }
    }
    pub(super) fn resume(now: u64) {
        // Spurious/other IRQs leave a running segment untouched.
        if !unsafe { core::mem::replace(&mut *(&raw mut RETURN_PENDING), false) } { return; }
        let id = policy().current().unwrap_or_else(|| fail(20));
        clock().resume(id, now).unwrap_or_else(|_| fail(20));
        unsafe { START = Some(now); }
        update_compare(now);
        if let Some(i) = unsafe { RETURN_ROW } {
            unsafe {
                ROWS[i][25] = COMPARE.unwrap_or(0);
                EVENTS[i][8] = clock().logical_end().unwrap_or(0);
                EVENTS[i][9] = COMPARE.unwrap_or(0);
            }
        }
    }
    pub(super) fn dump() {
        print(b"NATIVE THREAD RR CONFIG", &[unsafe { CLOCK_DOMAIN.1 }, NATIVE_QUANTUM_TICKS]);
        unsafe {
            for i in 0..USED { print(b"NATIVE THREAD RR EVENT", &*(&raw const EVENTS[i])); }
        }
        print(b"NATIVE THREAD RR SERVICE", &[clock().service(0), clock().service(1)]);
    }
}

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

fn update_compare(_now: u64) {
    #[cfg(jnox_native_thread_rr)]
    let next = rr::compare(_now);
    #[cfg(not(jnox_native_thread_rr))]
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
        State::MutexWait(token) => (7, token),
    }
}

fn record(now: u64, cause: u64, prior: Option<usize>, op: u64, args: [u64; 3],
          outgoing: Option<&NativeFrame>, selected: Option<&NativeFrame>, old_compare: Option<u64>) {
    #[cfg(jnox_native_thread_rr)]
    rr::selected(selected.is_some());
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
    let now = counter();
    #[cfg(jnox_native_thread_rr)]
    rr::resume(now);
    unsafe {
        if let Some(index) = (*(&raw mut RETURN_ROW)).take() { ROWS[index][27] = now; }
    }
}

pub(super) fn start(mut main: NativeFrame) -> u64 {
    main.tls[1] = locator(0);
    validate(0, &main);
    unsafe { SAVED[0] = main; }
    let now = counter();
    #[cfg(jnox_native_thread_rr)]
    { rr::init(); rr::enter(now, None); }
    record(now, 0, None, 0, [0; 3], None, Some(&main), None);
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

pub(super) fn irq(now: u64, frame: Option<&mut NativeFrame>) -> bool {
    let prior = policy().current();
    let old_compare = unsafe { COMPARE };
    if let Some(f) = frame.as_deref() {
        let id = prior.unwrap_or_else(|| fail(20));
        validate(id, f);
        unsafe { SAVED[id] = *f; }
    }
    #[cfg(jnox_native_thread_rr)]
    { rr::enter(now, prior); rr::timer(now); }
    #[cfg(not(jnox_native_thread_rr))]
    policy().wake_due(now);
    update_compare(now);
    let next = policy().current();
    let selected = next.map(saved);
    record(now, if frame.is_some() { 2 } else { 3 }, prior, 0, [0; 3], frame.as_deref(), selected, old_compare);
    if let Some(f) = frame {
        if prior != next {
            let id = next.unwrap_or_else(|| fail(20));
            validate(id, saved(id));
            *f = *saved(id);
        }
    }
    next.is_some()
}

// Rows share the index and loss accounting of the main thread journal.
static mut MUTEX_ROWS: [[u64; 12]; LIMIT] = [[0; 12]; LIMIT];

fn mutex_snapshot(addr: u64) -> [u64; 6] {
    for (index, slot) in policy().mutex_slots().iter().enumerate() {
        if slot.address == Some(addr) {
            return [index as u64, slot.generation,
                slot.owner.map_or(NO_PID, |t| t.tid as u64), slot.owner.map_or(0, |t| t.generation),
                slot.waiter.map_or(NO_PID, |t| t.tid as u64), slot.waiter.map_or(0, |t| t.generation)];
        }
    }
    [NO_PID, 0, NO_PID, 0, NO_PID, 0]
}

fn mutex_call(f: &mut NativeFrame, op: u64, addr: u64, token: u64) {
    let before = mutex_snapshot(addr);
    let valid = addr != 0 && addr % 8 == 0 && native_el0_mmu::user_range(addr as usize, 8, true);
    let result = if valid { policy().mutex(op, addr, token) } else { Err(22) };
    match result {
        Ok(value) => {
            f.x[0] = 0;
            if op == 13 { f.x[1] = value; }
            if op == 17 && before[4] != NO_PID {
                // The policy transferred ownership to this incarnation before
                // queuing it. Complete its blocked SVC in its own saved frame.
                unsafe { SAVED[before[4] as usize].x[0] = 0; }
            }
        }
        Err(errno) => f.x[0] = (-i64::from(errno)) as u64,
    }
    let index = unsafe { USED };
    if index < LIMIT {
        let after = mutex_snapshot(addr);
        unsafe {
            MUTEX_ROWS[index][..6].copy_from_slice(&before);
            MUTEX_ROWS[index][6..].copy_from_slice(&after);
        }
    }
}

pub(super) fn syscall(now: u64, f: &mut NativeFrame) {
    let id = policy().current().unwrap_or_else(|| fail(20));
    validate(id, f);
    let old_compare = unsafe { COMPARE };
    let [a, b, c] = [f.x[0], f.x[1], f.x[2]];
    let op = f.x[8];
    #[cfg(jnox_native_thread_rr)]
    rr::enter(now, Some(id));
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
        9 | 18 => f.x[0] = (-95i64) as u64,
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
        13..=17 => mutex_call(f, op, a, b),
        _ => f.x[0] = (-38i64) as u64,
    }
    unsafe { SAVED[id] = *f };
    #[cfg(jnox_native_thread_rr)]
    if op == 8 || policy().current() != Some(id) { rr::end_turn(); }
    update_compare(now);
    record(now, 1, Some(id), op, [a, b, c], Some(f), policy().current().map(saved), old_compare);
    if let Some(status) = policy().status() { finish(status); }
    if policy().current().is_none() {
        // An orphaned non-robust mutex or dependency cycle may have no
        // future timer event. The idle continuation then stays asleep.
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
        for i in 0..USED {
            print(b"NATIVE THREAD EVENT", &*(&raw const ROWS[i]));
            if (13..=17).contains(&ROWS[i][4]) {
                uart_write(b"NATIVE THREAD MUTEX ");
                unsigned(i as u64);
                print(b"", &*(&raw const MUTEX_ROWS[i]));
            }
        }
        print(b"NATIVE THREAD DROPPED", &[LOST]);
    }
    #[cfg(jnox_native_thread_rr)]
    rr::dump();
    uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
    park()
}
