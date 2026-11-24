//! Ordinary 1:2 CPU workloads on the shared exception-return ABI.
use super::context::*;
use super::fair_policy::VirtualTime;
use super::vruntime_policy::{Lease, VruntimeScheduler};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::sync::atomic::{AtomicU64, Ordering::SeqCst};

global_asm!(".if {trace}", ".set JNOX_WEIGHTED_TRACE, 1", ".endif",include_str!("context.S"), include_str!("fixed_runtime.S"),
    trace = const cfg!(jnox_weighted_trace) as usize, frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym irq, svc = sym svc, fail = sym fail);
unsafe extern "C" {
    fn fixed_runtime_enter();
}

const WAKE: bool = cfg!(jnox_weighted_wake);
const CAP: usize = 64;
const HORIZON: u64 = 36;
static mut STACKS: [Stack; 2] = [const { Stack([0; STACK_BYTES]) }; 2];
static mut STATE: Option<State> = None;
static PROGRESS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static HORIZON_COUNT: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static SOLO: AtomicU64 = AtomicU64::new(0);
static RESUMED: AtomicU64 = AtomicU64::new(0);
static WITNESS: AtomicU64 = AtomicU64::new(0);

fn check(ok: bool) {
    if !ok {
        fail()
    }
}
fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) }
    n
}
fn progress(id: usize) {
    // One writer per counter. Interrupts may occur between the load and store.
    let n = PROGRESS[id].load(SeqCst);
    PROGRESS[id].store(n.checked_add(1).unwrap_or_else(|| fail()), SeqCst);
}
#[inline(never)]
extern "C" fn weighted_a() {
    let mut wakes = 0;
    for _ in 0..u32::MAX {
        progress(0);
        if WAKE {
            if RESUMED.load(SeqCst) == 3 {
                return;
            }
            if GENERATION.load(SeqCst) > wakes && SOLO.load(SeqCst) >= 3 {
                let accepted: usize;
                unsafe { asm!("svc #3", inout("x0") 1usize => accepted) }
                check(accepted == 1);
                wakes += 1;
            }
        } else if HORIZON_COUNT.load(SeqCst) == HORIZON {
            return;
        }
    }
    fail()
}
#[inline(never)]
extern "C" fn weighted_b() {
    if WAKE {
        for cycle in 1..=3 {
            progress(1);
            let stamp;
            // ELR saved by SVC points directly at this counter read.
            unsafe { asm!("svc #2", "mrs {}, cntpct_el0", out(reg) stamp) }
            WITNESS.store(stamp, SeqCst);
            progress(1);
            RESUMED.store(cycle, SeqCst);
        }
        return;
    }
    for _ in 0..u32::MAX {
        progress(1);
        if HORIZON_COUNT.load(SeqCst) == HORIZON {
            return;
        }
    }
    fail()
}

fn twice(t: VirtualTime) -> u64 {
    let (whole, r, w) = t.parts();
    u64::try_from(2 * u128::from(whole) + u128::from(2 * r / w)).unwrap_or_else(|_| fail())
}
fn base(id: usize) -> usize {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add(id * STACK_BYTES)
        .addr()
}
fn frame_ok(id: usize, f: *mut Frame) {
    check(
        f.is_aligned()
            && (base(id)..=base(id) + STACK_BYTES - size_of::<Frame>()).contains(&f.addr()),
    );
}
#[repr(u64)]
#[derive(Clone, Copy, PartialEq)]
enum Cause {
    Timer,
    Exit,
    Block,
    Wake,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Row {
    restore_marker: u64,
    selection_begin: u64,
    selection_end: u64,
    seq: u64,
    id: u64,
    cause: u64,
    ready: u64,
    start: u64,
    due: u64,
    stop: u64,
    gap: u64,
    restored: u64,
    restored_pc: u64,
    saved: u64,
    saved_pc: u64,
    pre: [u64; 2],
    charged: [u64; 2],
    placed: [u64; 2],
    service: [u64; 2],
    floor_before: u64,
    floor_charged: u64,
    floor_after: u64,
    accepted: u64,
    progress: [u64; 2],
    generation: u64,
    solo: u64,
    witness: u64,
    resumed: u64,
}
// Static journal avoids consuming exception or boot stack. Append once per closure.
#[unsafe(no_mangle)]
static mut weighted_rows: [Row; CAP] = [const { unsafe { core::mem::zeroed() } }; CAP];
#[unsafe(no_mangle)]
static mut weighted_restore_counter: u64 = 0;
static COUNT: AtomicU64 = AtomicU64::new(0);
struct State {
    policy: VruntimeScheduler<2>,
    lease: Option<Lease>,
    frames: [*mut Frame; 2],
    ready: u64,
    exited: u64,
    q: u64,
    start: u64,
    due: u64,
    stop: u64,
    gap: u64,
    saved_pc: [u64; 2],
    selection_begin: u64,
    selection_end: u64,
    resume_start: u64,
    pc: u64,
    restored: usize,
    accepted: u64,
    sleep_service: [u64; 2],
    after_wake: u8,
}
impl State {
    fn runtimes(&self) -> [u64; 2] {
        core::array::from_fn(|id| twice(self.policy.runtime(id).unwrap()))
    }
    fn service(&self) -> [u64; 2] {
        core::array::from_fn(|id| self.policy.service(id).unwrap())
    }
    fn dispatch(&mut self) -> *mut Frame {
        self.selection_begin = counter();
        let lease = self.policy.dispatch().unwrap_or_else(|| fail());
        self.selection_end = counter();
        if self.after_wake == 1 {
            check(lease.task == 0);
            self.after_wake = 2;
        } else if self.after_wake == 2 {
            check(lease.task == 1);
            self.after_wake = 0;
        }
        let f = self.frames[lease.task];
        frame_ok(lease.task, f);
        self.pc = unsafe { (*f).elr };
        check(self.pc == self.saved_pc[lease.task]);
        self.restored = f.addr();
        self.lease = Some(lease);
        self.start = counter();
        self.gap = if self.stop == 0 {
            0
        } else {
            self.start.checked_sub(self.stop).unwrap_or_else(|| fail())
        };
        check(self.gap <= self.q / 4);
        if self.accepted != 0 && lease.task == 1 {
            check(
                self.start
                    .checked_sub(self.accepted)
                    .is_some_and(|v| v <= self.q * 7 / 4),
            );
            self.resume_start = self.start;
            self.accepted = 0;
        }
        self.due = self.start.checked_add(self.q).unwrap_or_else(|| fail());
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb", in(reg) self.due, in(reg) 1u64)
        }
        f
    }
    fn close(&mut self, f: *mut Frame, stop: u64, cause: Cause) -> *mut Frame {
        let lease = self.lease.unwrap_or_else(|| fail());
        let id = lease.task;
        frame_ok(id, f);
        let ticks = stop.checked_sub(self.start).unwrap_or_else(|| fail());
        let pre = self.runtimes();
        let floor_before = twice(self.policy.floor());
        let ready = self.ready;
        check(self.policy.charge(lease, ticks));
        self.lease = None;
        self.stop = stop;
        self.frames[id] = f;
        self.saved_pc[id] = unsafe { (*f).elr };
        let charged = self.runtimes();
        let floor_charged = twice(self.policy.floor());
        let service = self.service();
        let mut accepted = 0;
        match cause {
            Cause::Block => {
                check(WAKE && id == 1 && ready == 3 && GENERATION.load(SeqCst) < 3);
                check(RESUMED.load(SeqCst) == GENERATION.load(SeqCst));
                check(self.policy.sleep(id));
                self.ready = 1;
                self.sleep_service = service;
                SOLO.store(0, SeqCst);
                GENERATION.fetch_add(1, SeqCst);
            }
            Cause::Wake => {
                check(WAKE && id == 0 && ready == 1 && unsafe { (*f).gpr[0] } == 1);
                check((3..=4).contains(&SOLO.load(SeqCst)) && service[1] == self.sleep_service[1]);
                check(
                    service[0] - self.sleep_service[0] >= 3 * self.q && charged[1] < floor_charged,
                );
                check(charged[0] == floor_charged && self.policy.wake(1));
                accepted = counter();
                self.accepted = accepted;
                self.after_wake = 1;
                check(self.runtimes()[1] == floor_charged && self.service() == service);
                self.ready = 3;
                unsafe { (*f).gpr[0] = 1 }
            }
            Cause::Exit => {
                check(if WAKE {
                    RESUMED.load(SeqCst) == 3
                } else {
                    HORIZON_COUNT.load(SeqCst) == HORIZON
                });
                check(self.policy.sleep(id));
                self.ready &= !(1 << id);
                self.exited |= 1 << id;
            }
            Cause::Timer if WAKE && ready == 1 && self.exited == 0 => {
                check(id == 0 && SOLO.fetch_add(1, SeqCst) < 4);
            }
            Cause::Timer if !WAKE && HORIZON_COUNT.load(SeqCst) < HORIZON => {
                let d = 2 * i128::from(service[0]) - i128::from(service[1]);
                let h = i128::from(self.q * 5 / 4);
                check(ready == 3 && (-h..=2 * h).contains(&d));
                if HORIZON_COUNT.fetch_add(1, SeqCst) + 1 == HORIZON {
                    let total = i128::from(service[0]) + i128::from(service[1]);
                    check(
                        total >= 36 * i128::from(self.q)
                            && d.abs() * 36 * i128::from(self.q) <= 2 * h * total,
                    );
                }
            }
            Cause::Timer => (),
        }
        let n = COUNT.load(SeqCst) as usize;
        check(n < CAP);
        let row = Row {
            restore_marker: unsafe { (&raw const weighted_restore_counter).read_volatile() },
            selection_begin: self.selection_begin,
            selection_end: self.selection_end,
            seq: lease.sequence,
            id: id as u64,
            cause: cause as u64,
            ready,
            start: self.start,
            due: self.due,
            stop,
            gap: self.gap,
            restored: self.restored as u64,
            restored_pc: self.pc,
            saved: f.addr() as u64,
            saved_pc: unsafe { (*f).elr },
            pre,
            charged,
            placed: self.runtimes(),
            service,
            floor_before,
            floor_charged,
            floor_after: twice(self.policy.floor()),
            accepted,
            progress: core::array::from_fn(|i| PROGRESS[i].load(SeqCst)),
            generation: GENERATION.load(SeqCst),
            solo: SOLO.load(SeqCst),
            witness: WITNESS.load(SeqCst),
            resumed: RESUMED.load(SeqCst),
        };
        unsafe { (&raw mut weighted_rows).cast::<Row>().add(n).write(row) };
        COUNT.store(n as u64 + 1, SeqCst);
        check(ticks <= self.q * 5 / 4 && (cause != Cause::Timer || ticks >= self.q));
        if WAKE && id == 1 && self.resume_start != 0 && row.witness >= self.resume_start {
            check(row.witness - self.resume_start <= self.q / 4);
            self.resume_start = 0;
        }
        if self.ready == 0 {
            check(self.resume_start == 0);
            check(self.exited == 3 && self.lease.is_none() && self.policy.dispatch().is_none());
            check(row.progress.iter().all(|&v| v > 0));
            report();
            super::uart_write(b"WEIGHTED TERMINAL\n");
            super::park()
        }
        self.dispatch()
    }
}

fn number(mut n: u64) {
    let mut digits = [0; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    super::uart_write(&digits[i..]);
}
fn line(tag: &[u8], values: &[u64]) {
    super::uart_write(tag);
    for &v in values {
        super::uart_write(b" ");
        number(v);
    }
    super::uart_write(b"\n");
}
fn report() {
    for i in 0..COUNT.load(SeqCst) as usize {
        let r = unsafe { (&raw const weighted_rows).cast::<Row>().add(i).read() };
        line(
            b"W",
            &[
                r.seq,
                r.id,
                r.cause,
                r.ready,
                r.start,
                r.due,
                r.stop,
                r.gap,
                r.pre[0],
                r.pre[1],
                r.charged[0],
                r.charged[1],
                r.placed[0],
                r.placed[1],
                r.service[0],
                r.service[1],
                r.floor_before,
                r.floor_charged,
                r.floor_after,
                r.accepted,
                r.progress[0],
                r.progress[1],
                r.generation,
                r.solo,
                r.witness,
                r.resumed,
                r.selection_begin,
                r.selection_end,
            ],
        );
        if cfg!(jnox_weighted_trace) {
            line(
                b"F",
                &[
                    r.seq,
                    r.restored,
                    r.restored_pc,
                    r.saved,
                    r.saved_pc,
                    r.restore_marker,
                ],
            );
        }
    }
}
extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", "msr cntp_ctl_el0, xzr", "isb") }
    report();
    super::uart_write(b"WEIGHTED FAIL\n");
    super::park()
}
fn disable() {
    unsafe { asm!("msr cntp_ctl_el0, xzr", "isb", "dsb sy") }
}
fn event(f: *mut Frame, now: u64, cause: Cause) -> *mut Frame {
    // Boot/handlers on CPU0 have IRQ masked. Only ordinary entries read atomics.
    unsafe {
        (&mut *(&raw mut STATE))
            .as_mut()
            .unwrap_or_else(|| fail())
            .close(f, now, cause)
    }
}
extern "C" fn irq(f: *mut Frame) -> *mut Frame {
    let now = counter();
    let iar = gic_read(GICC + 0xc);
    let due: u64;
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_cval_el0","mrs {}, cntp_ctl_el0",out(reg) due,out(reg) ctl) }
    check(iar & 1023 == TIMER_IRQ && ctl & 7 == 5);
    check(unsafe { (&*(&raw const STATE)).as_ref().unwrap().due } == due);
    disable();
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy") }
    event(f, now, Cause::Timer)
}
extern "C" fn svc(f: *mut Frame) -> *mut Frame {
    let now = counter();
    disable();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1",out(reg) esr) }
    check(esr >> 26 == 0x15);
    let cause = match esr & 0xffff {
        1 => Cause::Exit,
        2 => Cause::Block,
        3 => Cause::Wake,
        _ => fail(),
    };
    event(f, now, cause)
}
pub(super) fn run() -> ! {
    init_timer_irq();
    let hz: u64;
    unsafe { asm!("mrs {}, cntfrq_el0",out(reg) hz) }
    check(hz > 0 && hz % 400 == 0);
    let entries = [weighted_a as *const (), weighted_b as *const ()];
    let frames = core::array::from_fn(|id| {
        let f = (base(id) + STACK_BYTES - size_of::<Frame>()) as *mut Frame;
        unsafe {
            (*f).elr = fixed_runtime_enter as *const () as u64;
            (*f).spsr = 0x345;
            (*f).gpr[0] = entries[id] as u64;
        }
        f
    });
    line(
        b"WEIGHTED1",
        &[
            u64::from(WAKE),
            hz,
            hz / 100,
            CAP as u64,
            base(0) as u64,
            base(1) as u64,
            STACK_BYTES as u64,
            size_of::<Frame>() as u64,
        ],
    );
    let state = State {
        policy: VruntimeScheduler::new([1, 2]).unwrap_or_else(|| fail()),
        lease: None,
        frames,
        ready: 3,
        exited: 0,
        q: hz / 100,
        start: 0,
        due: 0,
        stop: 0,
        gap: 0,
        pc: 0,
        restored: 0,
        saved_pc: [fixed_runtime_enter as *const () as u64; 2],
        selection_begin: 0,
        selection_end: 0,
        resume_start: 0,
        accepted: 0,
        sleep_service: [0; 2],
        after_wake: 0,
    };
    unsafe {
        STATE = Some(state);
        sched_start_frame((&mut *(&raw mut STATE)).as_mut().unwrap().dispatch())
    }
}
