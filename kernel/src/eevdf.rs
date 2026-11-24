//! A finite live EEVDF workload. Each timer enforces one declared dispatch
//! interval. Whole physical-timer quanta charge the executing request. The
//! sleep syscalls run in a separate, untimed control interval after its
//! preceding service has retired. Each wake latency ends in task code.
use super::context::*;
use super::eevdf_policy::Eevdf;
use super::{park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

const DYNAMIC: bool = cfg!(jnox_bench = "eevdf_dynamic");
const DISTRIBUTION: bool = DYNAMIC || cfg!(jnox_bench = "eevdf_wake");
const SCALE: u64 = if DYNAMIC { 60 } else { 20 };
const ROWS: usize = if DYNAMIC {
    256
} else if DISTRIBUTION {
    128
} else {
    12
};
const WAKES: usize = if DYNAMIC {
    20
} else if DISTRIBUTION {
    16
} else {
    2
};
const C: usize = 2;
const OWNERS: [usize; 12] = [0, 1, 2, 1, 0, 0, 2, 1, 1, 0, 1, 2];
const GRANTS: [u64; 12] = [1, 1, 1, 3, 2, 2, 1, 1, 1, 2, 2, 1];
const TAGS: [u64; 3] = [0xee0a, 0xee0b, 0xee0c];
static COMMAND: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static OBSERVED: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut ACTIVE: Option<Bench> = None;

unsafe extern "C" {
    fn ee_a();
    fn ee_b();
    fn ee_c();
    static ee_a_work: u8;
    static ee_a_svc: u8;
    static ee_b_work: u8;
    static ee_b_svc: u8;
    static ee_c_work: u8;
    static ee_c_svc: u8;
    static ee_c_wake: u8;
    #[cfg(jnox_bench = "eevdf_dynamic")]
    static ee_c_config_return: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("eevdf.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym sleep_call, fail = sym fail,
    dynamic = const DYNAMIC as usize, command = sym COMMAND, observation = sym OBSERVED,
);

// Each task tuple is runtime numerator, deadline numerator, weight and
// remaining request. Rational timestamps use the task's weight as denominator.
type Tasks = [[u64; 4]; 3];
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Row {
    owner: u64,
    quota: u64,
    ready: u64,
    reference: [u64; 2],
    before: Tasks,
    start: u64,
    compare: u64,
    irq: u64,
    pc: u64,
    frame: u64,
    restore: u64,
    progress: u64,
    after: Tasks,
    error_scaled: [i64; 3],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Wake {
    sleep_after: u64,
    svc_at: u64,
    svc_return: u64,
    svc_frame: u64,
    wake_before: u64,
    wake_at: u64,
    prewake_v: [u64; 2],
    first_row: u64,
    first_tick: u64,
    first_work: u64,
    wait_units: u64,
}
#[repr(C)]
struct Witness {
    header: [u64; 8],
    rows: [Row; ROWS],
    wakes: [Wake; WAKES],
}
const _: () = assert!(
    size_of::<Row>() == 39 * 8
        && size_of::<Wake>() == 12 * 8
        && size_of::<Witness>() == 64 + ROWS * 312 + WAKES * 96
);
#[unsafe(no_mangle)]
static mut eevdf_witness: Witness = unsafe { core::mem::zeroed() };

#[cfg(jnox_bench = "eevdf_dynamic")]
#[repr(C)]
#[derive(Clone, Copy)]
struct Update {
    meta: [u64; 13],
    before: Tasks,
    after: Tasks,
}
#[cfg(jnox_bench = "eevdf_dynamic")]
#[unsafe(no_mangle)]
static mut eevdf_updates: [Update; 4] = unsafe { core::mem::zeroed() };

struct Bench {
    updates: usize,
    policy: Eevdf<3>,
    frequency: u64,
    quantum: u64,
    row: usize,
    total: u64,
    ready: u64,
    frames: [*mut Frame; 3],
    progress: [u64; 3],
    snapshot: Row,
    error_scaled: [i64; 3],
    peak: u64,
    wakes: [Wake; WAKES],
    sleeps: usize,
    responses: usize,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"EEVDF FAIL\n");
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
    require(id <= C);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

fn with_bench<T>(f: impl FnOnce(&mut Bench) -> T) -> T {
    let mask: u64;
    unsafe { asm!("mrs {}, daif", out(reg) mask, options(nostack, preserves_flags)) };
    require(mask & 0x80 != 0);
    // The sole CPU owns this state at boot or on masked exception entry.
    // All borrows end before exception return. Tasks access only atomics.
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(frequency: u64) -> Self {
        let entries: [unsafe extern "C" fn(); 3] = [ee_a, ee_b, ee_c];
        let frames = core::array::from_fn(|id| unsafe {
            let f = top(id).sub(size_of::<Frame>()).cast::<Frame>();
            (*f).elr = entries[id] as usize as u64;
            (*f).spsr = 0x3c5;
            (*f).gpr[19] = TAGS[id];
            (*f).gpr[21] = id as u64;
            f
        });
        let mut policy = if DISTRIBUTION {
            Eevdf::new([2, 2, 1], [1; 3])
        } else {
            Eevdf::new([1, 2, 1], [1, 8, 4])
        }
        .unwrap_or_else(|| fail());
        for id in 0..3 {
            require(policy.wake(id));
        }
        require(frequency >= 100);
        Self {
            updates: 0,
            policy,
            frequency,
            quantum: frequency / 100,
            row: 0,
            total: 0,
            ready: 7,
            frames,
            progress: [0; 3],
            snapshot: Row::default(),
            error_scaled: [0; 3],
            peak: 0,
            wakes: [Wake::default(); WAKES],
            sleeps: 0,
            responses: 0,
        }
    }

    fn tasks(&self) -> Tasks {
        core::array::from_fn(|id| {
            let (n, d, w) = self.policy.snapshot(id).unwrap_or_else(|| fail());
            [
                n,
                d,
                w.into(),
                self.policy.remaining(id).unwrap_or_else(|| fail()).into(),
            ]
        })
    }

    fn is_response(&self, id: usize) -> bool {
        if DISTRIBUTION {
            id == C && self.responses < self.sleeps
        } else {
            matches!(self.row, 6 | 11)
        }
    }

    fn wake_c(&mut self, index: usize) {
        let (n, d) = self.policy.reference().unwrap_or_else(|| fail());
        require(self.sleeps == index + 1 && self.responses == index && self.ready == 3);
        let before = self.tasks()[C];
        let wake_at = counter();
        require(self.policy.wake(C));
        self.ready |= 1 << C;
        let after = self.tasks()[C];
        if !DISTRIBUTION && index == 0 {
            require((n, d) == (6, 4) && before == [1, 2, 1, 1] && after == [2, 3, 1, 1]);
        } else if !DISTRIBUTION {
            require((n, d) == (11, 4) && before == [3, 5, 1, 2] && after == before);
        }
        let wake = &mut self.wakes[index];
        wake.wake_before = self.row as u64;
        wake.wake_at = wake_at;
        wake.prewake_v = [n, d];
    }

    fn dispatch(&mut self) -> *mut Frame {
        if DISTRIBUTION {
            if self.ready == 3 {
                let index = self.sleeps - 1;
                let due = self.wakes[index].sleep_after as usize + 2 + index % 4;
                if self.row == due {
                    self.wake_c(index);
                }
            }
        } else {
            match self.row {
                2 => {
                    require(self.policy.configure(C, 1, 2) && self.tasks()[C] == [0, 2, 1, 2]);
                    uart_write(b"EEVDF SLICE CHANGE\n");
                }
                3 => {
                    require(self.policy.reference() == Some((2, 3)));
                    require(self.policy.configure(0, 2, 2) && self.tasks()[0] == [2, 4, 2, 2]);
                    uart_write(b"EEVDF WEIGHT CHANGE\n");
                }
                4 => self.wake_c(0),
                8 => self.wake_c(1),
                _ => {}
            }
        }
        let (n, d) = self.policy.reference().unwrap_or_else(|| fail());
        let before = self.tasks();
        let id = self.policy.dispatch().unwrap_or_else(|| fail());
        require((DISTRIBUTION || id == OWNERS[self.row]) && self.ready & (1 << id) != 0);
        if !DISTRIBUTION && self.row == 1 {
            require((n, d) == (1, 4) && before == [[1, 2, 1, 1], [0, 8, 2, 8], [0, 4, 1, 4]]);
            uart_write(b"EEVDF EARLY INELIGIBLE\n");
        }
        let quota = if DISTRIBUTION { 1 } else { GRANTS[self.row] };
        require(quota <= before[id][3]);
        let f = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!f.is_null());
        let restore = unsafe { (*f).elr };
        if self.progress[id] != 0 {
            require(
                f.addr() == top(id).addr() - 16 - size_of::<Frame>()
                    && unsafe { (*f).gpr[20] } == self.progress[id],
            );
        }
        if self.is_response(id) {
            require(id == C && restore == (&raw const ee_c_wake).addr() as u64);
            let wake = &mut self.wakes[self.responses];
            wake.first_row = self.row as u64;
            wake.first_tick = self.total;
        }
        self.snapshot = Row {
            owner: id as u64,
            quota,
            ready: self.ready,
            reference: [n, d],
            before,
            restore,
            ..Row::default()
        };
        self.snapshot.start = counter();
        let compare = self
            .snapshot
            .start
            .checked_add(self.quantum.checked_mul(quota).unwrap_or_else(|| fail()))
            .unwrap_or_else(|| fail());
        self.snapshot.compare = compare;
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
        };
        f
    }

    fn check_frame(&self, id: usize, frame: *mut Frame) {
        let sp = top(id).addr() - 16;
        require(frame.addr() == sp - size_of::<Frame>() && self.frames[id].is_null());
        let s = unsafe { &*frame };
        require(s.spsr & 0x8f == 5 && s.gpr[19] == TAGS[id] && s.gpr[21] == id as u64);
        require(s.tpidr_el1 == TAGS[id] && s.simd[31] == [TAGS[id]; 2]);
        require(unsafe { ptr::read_volatile(sp as *const u64) } == TAGS[id]);
    }

    fn retire(&mut self, frame: *mut Frame, compare: u64, irq: u64) -> *mut Frame {
        let id = self.snapshot.owner as usize;
        require(compare == self.snapshot.compare && irq >= compare && irq - compare < self.quantum);
        let units = (irq - self.snapshot.start) / self.quantum;
        require(units == self.snapshot.quota && self.policy.stop(id, units as u32));
        self.check_frame(id, frame);
        let saved = unsafe { &*frame };
        let (begin, end) = match id {
            0 => (&raw const ee_a_work, &raw const ee_a_svc),
            1 => (&raw const ee_b_work, &raw const ee_b_svc),
            _ => (&raw const ee_c_work, &raw const ee_c_svc),
        };
        require((begin.addr()..end.addr()).contains(&(saved.elr as usize)));
        require(saved.gpr[20] > 1 && saved.gpr[20] > self.progress[id]);
        self.progress[id] = saved.gpr[20];
        self.frames[id] = frame;
        self.snapshot.irq = irq;
        self.snapshot.pc = saved.elr;
        self.snapshot.frame = frame.addr() as u64;
        self.snapshot.progress = saved.gpr[20];
        self.snapshot.after = self.tasks();
        // GPS comparison uses the runnable weights of this grant. The only
        // scale is divisible by every active weight sum in this fixture.
        let weight_sum = self.snapshot.reference[1];
        require(weight_sum > 0 && SCALE % weight_sum == 0);
        for task in 0..3 {
            if self.ready & (1 << task) != 0 {
                let ideal = SCALE * units * self.snapshot.before[task][2] / weight_sum;
                self.error_scaled[task] -= ideal as i64;
            }
        }
        self.error_scaled[id] += (SCALE * units) as i64;
        self.snapshot.error_scaled = self.error_scaled;
        for e in self.error_scaled {
            self.peak = self.peak.max(e.unsigned_abs());
        }
        require(self.peak <= if DISTRIBUTION { 20 * SCALE } else { 25 });
        // The QMP observer reads the symbol only after the terminal marker.
        unsafe { ptr::write_volatile(&raw mut eevdf_witness.rows[self.row], self.snapshot) };
        if self.is_response(id) {
            let ran = OBSERVED[C].load(Ordering::Acquire);
            let index = self.responses;
            let wake = &mut self.wakes[index];
            require(
                wake.wake_at <= self.snapshot.start
                    && (self.snapshot.start..compare).contains(&ran),
            );
            wake.first_work = ran;
            let wake_tick = if DISTRIBUTION {
                wake.wake_before
            } else {
                GRANTS[..wake.wake_before as usize].iter().sum::<u64>()
            };
            wake.wait_units = self.total - wake_tick;
            if DISTRIBUTION {
                let (grants, quanta) = if DYNAMIC { (8, 10) } else { (6, 8) };
                require(wake.wait_units <= grants && ran - wake.wake_at <= quanta * self.quantum);
            } else {
                require(wake.wait_units == [4, 5][index]);
            }
            self.responses += 1;
        }
        self.row += 1;
        self.total += units;
        let should_block = if DISTRIBUTION {
            id == C && self.sleeps < WAKES
        } else {
            self.row == 3 || self.row == 7
        };
        if should_block {
            // Return to the just-interrupted C without a compute grant. Its
            // next loop observation requests blocking using the real SVC vector.
            require(id == C);
            COMMAND[C].store(1, Ordering::Release);
            return core::mem::replace(&mut self.frames[C], ptr::null_mut());
        }
        if self.row == ROWS {
            if DISTRIBUTION {
                require(self.total == ROWS as u64 && (!DYNAMIC || self.updates == 4));
            } else {
                require(self.total == 18 && self.peak == 25 && self.error_scaled == [-3, 2, 1]);
            }
            require(
                self.sleeps == WAKES
                    && self.responses == WAKES
                    && self.frames.iter().all(|f| !f.is_null()),
            );
            let header = [
                1,
                self.row as u64,
                self.sleeps as u64,
                self.responses as u64,
                self.frequency,
                self.quantum,
                self.total,
                self.peak,
            ];
            unsafe {
                ptr::write_volatile(&raw mut eevdf_witness.wakes, self.wakes);
                ptr::write_volatile(&raw mut eevdf_witness.header, header);
                asm!("dsb sy", options(nostack, preserves_flags));
            }
            if DYNAMIC {
                uart_write(b"EEVDF DYNAMIC STOP\nEEVDF DYNAMIC PASS\n");
            } else if DISTRIBUTION {
                uart_write(b"EEVDF WAKE STOP\nEEVDF WAKE PASS\n");
            } else {
                uart_write(b"EEVDF TWO WAKES\nEEVDF STOP\nEEVDF PASS\n");
            }
            park()
        }
        self.dispatch()
    }

    #[cfg(jnox_bench = "eevdf_dynamic")]
    fn update(&mut self, frame: *mut Frame, now: u64) -> *mut Frame {
        self.check_frame(C, frame);
        let saved = unsafe { &*frame };
        let sleep = self.sleeps + 1;
        let requests = [
            (5, 2, 3, true),
            (9, 0, 3, false),
            (13, 3, 3, true),
            (17, 3, 0, false),
        ];
        require(self.updates < requests.len());
        let (expected_sleep, weight, slice, expected) = requests[self.updates];
        require(self.row > 0 && self.snapshot.owner == C as u64 && sleep == expected_sleep);
        require(saved.elr == (&raw const ee_c_config_return).addr() as u64);
        require(saved.gpr[..3] == [0, weight, slice] && saved.gpr[26] == sleep as u64);
        require(COMMAND[C].load(Ordering::Acquire) == 1);
        let before = self.tasks();
        let (n, d) = self.policy.reference().unwrap_or_else(|| fail());
        let accepted = self.policy.configure(
            saved.gpr[0] as usize,
            saved.gpr[1] as u32,
            saved.gpr[2] as u32,
        );
        let after = self.tasks();
        let (nn, dd) = self.policy.reference().unwrap_or_else(|| fail());
        require(accepted == expected && after[1..] == before[1..]);
        if accepted {
            let start = (before[0][0] * d + (weight - before[0][2]) * n).div_ceil(d);
            require(after[0] == [start, start + slice, weight, slice]);
        } else {
            require(before == after && (n, d) == (nn, dd));
        }
        let record = Update {
            meta: [
                sleep as u64,
                self.row as u64,
                now,
                saved.elr,
                frame.addr() as u64,
                0,
                weight,
                slice,
                accepted as u64,
                n,
                d,
                nn,
                dd,
            ],
            before,
            after,
        };
        unsafe { ptr::write_volatile(&raw mut eevdf_updates[self.updates], record) };
        self.updates += 1;
        frame
    }

    fn block_c(&mut self, frame: *mut Frame, now: u64) -> *mut Frame {
        require(self.sleeps < WAKES && (DISTRIBUTION || self.row == [3, 7][self.sleeps]));
        self.check_frame(C, frame);
        let saved = unsafe { &*frame };
        require(saved.elr == (&raw const ee_c_wake).addr() as u64);
        require(saved.gpr[20] >= self.progress[C] && COMMAND[C].swap(0, Ordering::AcqRel) == 1);
        require(self.policy.sleep(C));
        self.ready &= !(1 << C);
        self.frames[C] = frame;
        self.progress[C] = saved.gpr[20];
        OBSERVED[C].store(0, Ordering::Release);
        self.wakes[self.sleeps] = Wake {
            sleep_after: (self.row - 1) as u64,
            svc_at: now,
            svc_return: saved.elr,
            svc_frame: frame.addr() as u64,
            ..Wake::default()
        };
        self.sleeps += 1;
        self.dispatch()
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency;
    unsafe { asm!("mrs {}, cntfrq_el0",out(reg) frequency, options(nostack,preserves_flags)) };
    uart_write(b"EEVDF BEGIN\n");
    unsafe {
        ACTIVE = Some(Bench::new(frequency));
        sched_start_frame(with_bench(Bench::dispatch))
    }
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let irq = counter();
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    let compare: u64;
    unsafe {
        asm!("mrs {}, cntp_ctl_el0","mrs {}, cntp_cval_el0",out(reg) ctl,out(reg) compare,options(nostack,preserves_flags))
    };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|b| b.retire(frame, compare, irq))
}

extern "C" fn sleep_call(frame: *mut Frame) -> *mut Frame {
    let now = counter();
    let esr: u64;
    let ctl: u64;
    unsafe {
        asm!("mrs {}, esr_el1","mrs {}, cntp_ctl_el0",out(reg) esr,out(reg) ctl,options(nostack,preserves_flags))
    };
    require(esr >> 26 == 0x15 && ctl & 1 == 0);
    #[cfg(jnox_bench = "eevdf_dynamic")]
    if esr & 0xffff == 2 {
        return with_bench(|b| b.update(frame, now));
    }
    require(esr & 0xffff == 1);
    with_bench(|b| b.block_c(frame, now))
}
