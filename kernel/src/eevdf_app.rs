//! Single-core ordinary EEVDF workload. Addressable raw records are defined in
//! docs/ordinary-eevdf-wire.md. All policy mutation is on masked vector entry.
use super::context::*;
use super::eevdf_app_tasks::{batch_a, batch_b, responder, BATCH_LIMIT};
use super::eevdf_policy::Eevdf;
use super::{park, uart_write};
use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const TASKS: usize = 3;
const CONSUMER: usize = 2;
const RELEASES: usize = 8;
const CAPACITY: usize = 512;

global_asm!(
    ".set JNOX_STAMP_X9, 1",
    include_str!("context.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym irq, svc = sym svc, fail = sym vector_fail,
);

#[repr(C)]
struct Header {
    version: AtomicU64,
    state: AtomicU64,
    failure: AtomicU64,
    frequency: AtomicU64,
    quantum: AtomicU64,
    origin: AtomicU64,
    rows: AtomicU64,
    wakes: AtomicU64,
    responses: AtomicU64,
    finished: AtomicU64,
    stack_bytes: AtomicU64,
    capacity: AtomicU64,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Row {
    ordinal: u64,
    owner: u64,
    ready_open: u64,
    start: u64,
    compare: u64,
    stop: u64,
    cause: u64,
    saved_frame: u64,
    saved_pc: u64,
    restored_frame: u64,
    restored_pc: u64,
    args: [u64; 4],
    woken: u64,
    wake_at: u64,
    ready_after: u64,
    reference: [u64; 2],
    iar: u64,
    tasks: [[u64; 4]; TASKS],
}

#[repr(C)]
struct Journal {
    header: Header,
    rows: UnsafeCell<[Row; CAPACITY]>,
    first: [AtomicU64; RELEASES],
}

// One masked CPU appends each row exactly once, then publishes its committed
// length. Workers access only atomic fields. The observer stops the guest.
unsafe impl Sync for Journal {}
const _: () = assert!(size_of::<Header>() == 96 && size_of::<Row>() == 264
    && size_of::<Journal>() == 96 + 512 * 264 + 64 && size_of::<Frame>() == 832);

#[unsafe(no_mangle)]
static jnox_eevdf_app_journal: Journal = unsafe { core::mem::zeroed() };
#[unsafe(no_mangle)]
static mut jnox_eevdf_app_stacks: [Stack; TASKS] = [const { Stack([0; STACK_BYTES]) }; TASKS];

static mut ACTIVE: Option<Runtime> = None;
static DRAIN: AtomicBool = AtomicBool::new(false);
static LAST_WAKE: AtomicU64 = AtomicU64::new(0);

fn header() -> &'static Header {
    &jnox_eevdf_app_journal.header
}

fn counter() -> u64 {
    let value;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn fail(code: u64) -> ! {
    unsafe { asm!("msr daifset, #0xf", "msr cntp_ctl_el0, xzr", "isb", options(nostack)) };
    header().failure.store(code, Ordering::Relaxed);
    header().state.store(3, Ordering::Release);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    uart_write(b"EEVDF APP FAIL\n");
    park()
}

extern "C" fn vector_fail() -> ! {
    fail(1)
}

fn require(condition: bool, code: u64) {
    if !condition {
        fail(code);
    }
}

pub(crate) fn drain_requested() -> bool {
    DRAIN.load(Ordering::Acquire)
}

#[inline(never)]
#[unsafe(export_name = "eevdf_app_wait")]
pub(crate) extern "C" fn wait(sequence: u64) -> u64 {
    let first;
    // The saved return PC is the ISB directly preceding this task's counter read.
    unsafe {
        asm!("svc #1", "isb", "mrs {}, cntpct_el0",
            lateout(reg) first, in("x0") sequence, options(nostack, preserves_flags));
    }
    first
}

pub(crate) fn responded(sequence: u64, first: u64) {
    require((1..=8).contains(&sequence)
        && header().wakes.load(Ordering::Acquire) == sequence
        && header().responses.load(Ordering::Relaxed) + 1 == sequence
        && first >= LAST_WAKE.load(Ordering::Acquire), 2);
    jnox_eevdf_app_journal.first[(sequence - 1) as usize].store(first, Ordering::Release);
    header().responses.store(sequence, Ordering::Release);
}

#[inline(never)]
#[unsafe(export_name = "eevdf_app_finish")]
pub(crate) extern "C" fn finish(id: u64, count: u64, sum: u64, weighted: u64) -> ! {
    unsafe {
        asm!("svc #2", "brk #0", in("x0") id, in("x1") count, in("x2") sum,
            in("x3") weighted, options(noreturn));
    }
}

fn stack_base(id: usize) -> usize {
    (&raw mut jnox_eevdf_app_stacks).cast::<u8>().wrapping_add(id * STACK_BYTES).addr()
}

fn check_frame(id: usize, frame: *mut Frame) {
    let base = stack_base(id);
    require(frame.is_aligned() &&
        (base..=base + STACK_BYTES - size_of::<Frame>()).contains(&frame.addr()), 3);
}

struct Runtime {
    policy: Eevdf<TASKS>,
    frames: [*mut Frame; TASKS],
    ready: u64,
    finished: u64,
    quantum: u32,
    origin: u64,
    wakes: u64,
    waiting: Option<u64>,
    row: Row,
    count: usize,
}

impl Runtime {
    fn new(quantum: u32) -> Self {
        let entries: [extern "C" fn() -> !; TASKS] = [batch_a, batch_b, responder];
        let frames = core::array::from_fn(|id| {
            let top = (&raw mut jnox_eevdf_app_stacks).cast::<u8>().wrapping_add((id + 1) * STACK_BYTES);
            let frame = top.wrapping_sub(size_of::<Frame>()).cast::<Frame>();
            unsafe {
                // The boot BSS supplies an otherwise zero frame at each stack top.
                (*frame).elr = entries[id] as usize as u64;
                (*frame).spsr = 0x345;
                (*frame).tpidr_el1 = id as u64 + 1;
            }
            frame
        });
        let mut policy = Eevdf::new([2, 1, 1], [2 * quantum, quantum, quantum / 2])
            .unwrap_or_else(|| fail(4));
        for id in 0..TASKS {
            require(policy.wake(id), 4);
        }
        Self { policy, frames, ready: 7, finished: 0, quantum, origin: 0, wakes: 0,
            waiting: None, row: Row::default(), count: 0 }
    }

    fn next_due(&self) -> Option<u64> {
        (self.wakes < RELEASES as u64).then(|| {
            self.origin.checked_add(8 * u64::from(self.quantum) * (self.wakes + 1))
                .unwrap_or_else(|| fail(5))
        })
    }

    fn dispatch(&mut self) -> *mut Frame {
        require(self.count < CAPACITY, 6);
        let (n, d) = self.policy.reference().unwrap_or_else(|| fail(7));
        let tasks = core::array::from_fn(|id| {
            let (runtime, deadline, weight) = self.policy.snapshot(id).unwrap_or_else(|| fail(7));
            let remaining = self.policy.remaining(id).unwrap_or_else(|| fail(7));
            [runtime, deadline, u64::from(weight), u64::from(remaining)]
        });
        let id = self.policy.dispatch().unwrap_or_else(|| fail(7));
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        check_frame(id, frame);
        require(self.ready & (1 << id) != 0, 7);
        let span = u64::from(self.quantum).min(tasks[id][3]);
        let after_wait = self.count != 0 && self.row.cause == 2;
        self.row = Row {
            ordinal: self.count as u64, owner: id as u64, ready_open: self.ready,
            restored_frame: frame.addr() as u64, restored_pc: unsafe { (*frame).elr },
            reference: [n, d], tasks, ..Row::default()
        };
        let start = counter();
        if self.count == 0 {
            self.origin = start;
            header().origin.store(start, Ordering::Release);
        }
        let end = start.checked_add(span).unwrap_or_else(|| fail(5));
        let compare = match self.next_due() {
            Some(due) => {
                // This next dispatch is an observable upper bound for the
                // preceding wait's admission into the sleeping set.
                require(!after_wait || start < due, 8);
                end.min(due)
            }
            None => end,
        };
        self.row.start = start;
        self.row.compare = compare;
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags));
        }
        frame
    }

    fn block(&mut self, seq: u64) {
        require(self.row.owner == CONSUMER as u64 && self.waiting.is_none()
            && seq == self.wakes + 1 && seq <= RELEASES as u64
            && header().responses.load(Ordering::Acquire) == seq - 1
            && self.row.stop < self.next_due().unwrap_or_else(|| fail(8)), 8);
        require(self.policy.sleep(CONSUMER), 8);
        self.waiting = Some(seq);
        self.ready &= !(1 << CONSUMER);
        if seq == 5 {
            require(self.policy.configure(1, 2, 2 * self.quantum), 9);
        }
    }

    fn release_if_due(&mut self) {
        let Some(due) = self.next_due() else { return };
        if self.row.stop < due {
            return;
        }
        let seq = self.wakes + 1;
        require(self.waiting == Some(seq) && self.policy.wake(CONSUMER), 10);
        let wake_at = counter();
        self.waiting = None;
        self.ready |= 1 << CONSUMER;
        self.wakes = seq;
        self.row.woken = seq;
        self.row.wake_at = wake_at;
        LAST_WAKE.store(wake_at, Ordering::Release);
        header().wakes.store(seq, Ordering::Release);
    }

    fn complete_task(&mut self, id: usize) {
        let [reported, count, sum, weighted] = self.row.args;
        require(reported == id as u64 && self.finished & (1 << id) == 0, 11);
        if id == CONSUMER {
            require(self.wakes == RELEASES as u64 && self.waiting.is_none()
                && header().responses.load(Ordering::Acquire) == RELEASES as u64
                && (count, sum, weighted) == (128, 27028, 115128), 12);
            DRAIN.store(true, Ordering::Release);
        } else {
            require(drain_requested() && (64..BATCH_LIMIT).contains(&count) && count % 64 == 0, 13);
        }
        require(self.policy.sleep(id), 11);
        self.finished |= 1 << id;
        self.ready &= !(1 << id);
        header().finished.store(u64::from(self.finished.count_ones()), Ordering::Release);
    }

    fn close(&mut self, frame: *mut Frame, cause: u64, iar: u32, compare: u64) -> *mut Frame {
        let id = self.row.owner as usize;
        check_frame(id, frame);
        let saved = unsafe { &*frame };
        require(self.frames[id].is_null() && saved.spsr & 0x8f == 5
            && saved.tpidr_el1 == id as u64 + 1 && self.row.compare == compare, 14);
        let stop = saved.reserved;
        let elapsed = stop.checked_sub(self.row.start).unwrap_or_else(|| fail(15));
        require((cause != 1 || stop >= compare) && self.policy.stop_elapsed(id, elapsed), 15);
        self.frames[id] = frame;
        self.row.cause = cause;
        self.row.stop = stop;
        self.row.saved_frame = frame.addr() as u64;
        self.row.saved_pc = saved.elr;
        self.row.args.copy_from_slice(&saved.gpr[..4]);
        self.row.iar = u64::from(iar);
        match cause {
            1 => self.release_if_due(),
            2 => self.block(self.row.args[0]),
            3 => self.complete_task(id),
            _ => fail(16),
        }
        self.row.ready_after = self.ready;
        unsafe { ptr::write_volatile(jnox_eevdf_app_journal.rows.get().cast::<Row>().add(self.count), self.row) };
        self.count += 1;
        header().rows.store(self.count as u64, Ordering::Release);
        if self.ready == 0 {
            require(self.finished == 7 && self.policy.dispatch().is_none(), 17);
            header().state.store(2, Ordering::Release);
            unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
            uart_write(b"EEVDF APP COMPLETE\n");
            park()
        }
        self.dispatch()
    }
}

fn runtime() -> &'static mut Runtime {
    // Only boot and this single CPU's masked handlers call this accessor.
    // The returned borrow ends before sched_start_frame switches stacks.
    unsafe { (*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail(18)) }
}

fn stop_timer() -> (u64, u64) {
    let compare;
    let control;
    unsafe {
        asm!("mrs {}, cntp_cval_el0", "mrs {}, cntp_ctl_el0", "msr cntp_ctl_el0, xzr",
            "isb", "dsb sy", out(reg) compare, out(reg) control, options(nostack, preserves_flags));
    }
    (compare, control)
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let (compare, control) = stop_timer();
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5, 19);
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    runtime().close(frame, 1, iar, compare)
}

extern "C" fn svc(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    let (compare, control) = stop_timer();
    require(esr >> 26 == 0x15 && control & 3 == 1, 20);
    let cause = match esr & 0xffff {
        1 => 2,
        2 => 3,
        _ => fail(20),
    };
    runtime().close(frame, cause, 0, compare)
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let h = header();
    h.version.store(1, Ordering::Relaxed);
    h.frequency.store(frequency, Ordering::Relaxed);
    h.stack_bytes.store(STACK_BYTES as u64, Ordering::Relaxed);
    h.capacity.store(CAPACITY as u64, Ordering::Relaxed);
    let quantum = frequency / 100;
    require(frequency != 0 && frequency % 200 == 0 && quantum <= u64::from(u32::MAX / 2), 21);
    h.quantum.store(quantum, Ordering::Relaxed);
    h.state.store(1, Ordering::Release);
    unsafe { ACTIVE = Some(Runtime::new(quantum as u32)) };
    uart_write(b"EEVDF APP BEGIN\n");
    unsafe { sched_start_frame(runtime().dispatch()) }
}
