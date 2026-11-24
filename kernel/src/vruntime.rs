//! Weighted virtual runtime over measured physical-counter intervals on one CPU.
//! Each charge includes the return to the selected task and the vector save
//! before the IRQ sample. Scheduler work between charges is outside the interval.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::fair_policy::VirtualTime;
use super::vruntime_policy::{Lease, VruntimeScheduler};
use super::{park, uart_write};

const A: usize = 0;
const B: usize = 1;
const SLEEP: usize = 12;
const WAKE: usize = 15;
const SLOTS: usize = 21;
const TAGS: [u64; 2] = [0x710a, 0x710b];
static mut STACKS: [Stack; 2] = [const { Stack([0; STACK_BYTES]) }; 2];
static mut ACTIVE: Option<Bench> = None;

unsafe extern "C" {
    fn vr_a();
    fn vr_b();
    static vr_a_begin: u8;
    static vr_a_end: u8;
    static vr_b_begin: u8;
    static vr_b_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("vruntime.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym fail, fail = sym fail,
);

#[derive(Clone, Copy, Default)]
struct Sample {
    task: usize,
    ready: u8,
    start: u64,
    compare: u64,
    irq: u64,
    charge: u64,
    runtime2: [u64; 2],
    floor2: u64,
    pc: u64,
    sp: usize,
    progress: u64,
}

struct Bench {
    scheduler: VruntimeScheduler<2>,
    frequency: u64,
    quantum: u64,
    ready: u8,
    slot: usize,
    lease: Option<Lease>,
    start: u64,
    frames: [*mut Frame; 2],
    pcs: [u64; 2],
    progress: [u64; 2],
    rows: [Sample; SLOTS],
    competitive: [u64; 2],
    sleep_runtime: u64,
    sleep_floor: u64,
    wake_before: u64,
    wake_floor: u64,
    wake_after: u64,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"VR FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let now;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) };
    now
}

fn top(id: usize) -> *mut u8 {
    require(id <= B);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// Twice virtual time is an exact integer for this workload's weights 1 and 2.
fn twice(v: VirtualTime) -> u64 {
    let (whole, numerator, denominator) = v.parts();
    require(denominator == 1 || denominator == 2);
    whole
        .checked_mul(2)
        .and_then(|n| n.checked_add(2 * numerator / denominator))
        .unwrap_or_else(|| fail())
}

fn with_bench<T>(f: impl FnOnce(&mut Bench) -> T) -> T {
    let mask: u64;
    unsafe { asm!("mrs {}, daif", out(reg) mask, options(nostack, preserves_flags)) };
    require(mask & 0x80 != 0);
    // The boot CPU and its masked exception handlers own the policy. No
    // borrow survives the exception return into an assembly work loop.
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(frequency: u64) -> Self {
        let entries: [unsafe extern "C" fn(); 2] = [vr_a, vr_b];
        let frames = core::array::from_fn(|id| unsafe {
            let frame = top(id).sub(size_of::<Frame>()).cast::<Frame>();
            (*frame).elr = entries[id] as usize as u64;
            (*frame).spsr = 0x3c5;
            (*frame).gpr[19] = TAGS[id];
            frame
        });
        require(frequency >= 50);
        Self {
            scheduler: VruntimeScheduler::new([1, 2]).unwrap_or_else(|| fail()),
            frequency,
            quantum: frequency / 50,
            ready: 3,
            slot: 0,
            lease: None,
            start: 0,
            frames,
            pcs: [0; 2],
            progress: [0; 2],
            rows: [Sample::default(); SLOTS],
            competitive: [0; 2],
            sleep_runtime: 0,
            sleep_floor: 0,
            wake_before: 0,
            wake_floor: 0,
            wake_after: 0,
        }
    }

    fn runtimes(&self) -> [u64; 2] {
        core::array::from_fn(|id| twice(self.scheduler.runtime(id).unwrap_or_else(|| fail())))
    }

    fn service(&self) -> [u64; 2] {
        core::array::from_fn(|id| self.scheduler.service(id).unwrap_or_else(|| fail()))
    }

    fn share_error(&self) -> u64 {
        let weighted_a = self.competitive[A].checked_mul(2).unwrap_or_else(|| fail());
        weighted_a.abs_diff(self.competitive[B])
    }

    fn tolerance(&self) -> u64 {
        self.quantum.checked_mul(4).unwrap_or_else(|| fail())
    }

    fn boundary_event(&mut self) {
        if self.slot == SLEEP {
            self.competitive = self.service();
            require(
                self.competitive.iter().all(|&s| s != 0) && self.share_error() <= self.tolerance(),
            );
            self.sleep_runtime = self.runtimes()[B];
            require(self.scheduler.sleep(B));
            self.ready = 1;
            self.sleep_floor = twice(self.scheduler.floor());
            require(self.sleep_floor == self.runtimes()[A]);
        } else if self.slot == WAKE {
            self.wake_before = self.runtimes()[B];
            self.wake_floor = twice(self.scheduler.floor());
            require(self.wake_before == self.sleep_runtime && self.wake_before < self.wake_floor);
            require(self.service()[B] == self.competitive[B] && self.scheduler.wake(B));
            self.ready = 3;
            self.wake_after = self.runtimes()[B];
            require(self.wake_after == self.wake_floor && self.service()[B] == self.competitive[B]);
        }
    }

    fn dispatch(&mut self) -> *mut Frame {
        require(self.lease.is_none());
        self.boundary_event();
        let lease = self.scheduler.dispatch().unwrap_or_else(|| fail());
        let id = lease.task;
        require(id <= B && self.ready & (1 << id) != 0);
        let before = self.runtimes();
        let expected = if self.ready == 1 || before[A] <= before[B] {
            A
        } else {
            B
        };
        require(id == expected);
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        if self.progress[id] != 0 {
            let saved = unsafe { &*frame };
            require(saved.elr == self.pcs[id] && saved.gpr[20] == self.progress[id]);
            require(frame.addr() == top(id).addr() - 16 - size_of::<Frame>());
        }
        self.lease = Some(lease);
        // Re-arm relative to this dispatch. Time spent in the previous
        // handler, including sleep/wake placement, is charged to neither task.
        self.start = counter();
        let compare = self
            .start
            .checked_add(self.quantum)
            .unwrap_or_else(|| fail());
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
        };
        frame
    }

    fn retire(&mut self, frame: *mut Frame, compare: u64, irq: u64) -> *mut Frame {
        let lease = self.lease.take().unwrap_or_else(|| fail());
        let id = lease.task;
        require(compare.checked_sub(self.start) == Some(self.quantum));
        require(irq >= compare && irq - compare < self.quantum);
        let charge = irq - self.start;
        let sp = top(id).addr() - 16;
        require(self.frames[id].is_null() && frame.addr() == sp - size_of::<Frame>());
        let saved = unsafe { &*frame };
        let (begin, end) = if id == A {
            (&raw const vr_a_begin, &raw const vr_a_end)
        } else {
            (&raw const vr_b_begin, &raw const vr_b_end)
        };
        let tag = TAGS[id];
        require((begin.addr()..end.addr()).contains(&(saved.elr as usize)));
        require(saved.spsr & 0x8f == 5 && saved.gpr[19] == tag && saved.tpidr_el1 == tag);
        require(
            saved.simd[31] == [tag; 2] && unsafe { ptr::read_volatile(sp as *const u64) } == tag,
        );
        require(saved.gpr[20] > 1 && saved.gpr[20] > self.progress[id]);
        self.progress[id] = saved.gpr[20];
        self.pcs[id] = saved.elr;
        self.frames[id] = frame;
        let before = self.runtimes();
        require(self.scheduler.charge(lease, charge));
        let after = self.runtimes();
        let increment = charge
            .checked_mul(if id == A { 2 } else { 1 })
            .unwrap_or_else(|| fail());
        require(
            after[id].checked_sub(before[id]) == Some(increment) && after[1 - id] == before[1 - id],
        );
        self.rows[self.slot] = Sample {
            task: id,
            ready: self.ready,
            start: self.start,
            compare,
            irq,
            charge,
            runtime2: after,
            floor2: twice(self.scheduler.floor()),
            pc: saved.elr,
            sp,
            progress: saved.gpr[20],
        };
        self.slot += 1;
        if self.slot == SLOTS {
            self.report();
            park()
        }
        self.dispatch()
    }

    fn report(&self) {
        let totals = self.service();
        let last = self.rows[SLOTS - 1];
        require(
            self.rows[WAKE..].iter().any(|r| r.task == A)
                && self.rows[WAKE..].iter().any(|r| r.task == B),
        );
        require(
            last.floor2 <= last.runtime2[B] && last.runtime2[B] - last.floor2 <= self.quantum * 2,
        );
        uart_write(b"VR CLOCK");
        field(b" freq=", self.frequency);
        field(b" quantum=", self.quantum);
        uart_write(b"\n");
        for (t, row) in self.rows.iter().enumerate() {
            uart_write(b"VR SLOT");
            field(b" t=", t as u64);
            field(b" id=", row.task as u64);
            field(b" ready=", row.ready.into());
            field(b" start=", row.start);
            field(b" cval=", row.compare);
            field(b" irq=", row.irq);
            field(b" charge=", row.charge);
            field(b" a2=", row.runtime2[A]);
            field(b" b2=", row.runtime2[B]);
            field(b" floor2=", row.floor2);
            field(b" elr=", row.pc);
            field(b" sp=", row.sp as u64);
            field(b" progress=", row.progress);
            uart_write(b"\n");
        }
        uart_write(b"VR SHARE slots=12");
        field(b" a=", self.competitive[A]);
        field(b" b=", self.competitive[B]);
        field(b" error=", self.share_error());
        field(b" tolerance=", self.tolerance());
        uart_write(b"\nVR SLEEP t=12 id=1");
        field(b" before2=", self.sleep_runtime);
        field(b" floor2=", self.sleep_floor);
        uart_write(b"\nVR WAKE t=15 id=1");
        field(b" before2=", self.wake_before);
        field(b" floor2=", self.wake_floor);
        field(b" after2=", self.wake_after);
        lag(b" lag_before=", self.wake_floor, self.wake_before);
        lag(b" lag_after=", self.wake_floor, self.wake_after);
        uart_write(b"\nVR END slots=21");
        field(b" a=", totals[A]);
        field(b" b=", totals[B]);
        field(b" floor2=", last.floor2);
        lag(b" lag_b=", last.floor2, last.runtime2[B]);
        uart_write(b"\nVR STOP\nVR PASS\n");
    }
}

fn number(mut n: u64) {
    let mut digits = [0; 20];
    let mut first = digits.len();
    loop {
        first -= 1;
        digits[first] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&digits[first..]);
}

fn field(name: &[u8], n: u64) {
    uart_write(name);
    number(n);
}

// For B, weight * (floor - runtime) in counter ticks equals floor2 - b2.
fn lag(name: &[u8], floor2: u64, b2: u64) {
    uart_write(name);
    if floor2 < b2 {
        uart_write(b"-")
    }
    number(floor2.abs_diff(b2));
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    uart_write(b"VR BEGIN\n");
    unsafe {
        ACTIVE = Some(Bench::new(frequency));
        sched_start_frame(with_bench(Bench::dispatch))
    }
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let irq = counter();
    let iar = gic_read(GICC + 0x0c);
    let compare: u64;
    let control: u64;
    unsafe {
        asm!("mrs {}, cntp_cval_el0", "mrs {}, cntp_ctl_el0",
        out(reg) compare, out(reg) control, options(nostack, preserves_flags))
    };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
    unsafe {
        asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy",
        in(reg) 2u64, options(nostack, preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|bench| bench.retire(frame, compare, irq))
}
