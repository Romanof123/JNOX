//! Two finite ticket lotteries and a measured-service stride run on one CPU.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::proportional_policy::Proportional;
use super::{park, uart_write};

const WEIGHTS: [u8; 3] = [1, 2, 4];
const SEEDS: [u64; 2] = [1, 2];
const WINDOW: usize = 32;
const STRIDE_SLOTS: usize = 16;
const ROWS: usize = 2 * WINDOW + STRIDE_SLOTS;
const NONE: u8 = 255;
const SCALE: u32 = 720_720;

unsafe extern "C" {
    fn prop_cpu_a();
    fn prop_cpu_b();
    fn prop_cpu_c();
    fn prop_yielder();
    static prop_cpu_a_work: u8;
    static prop_cpu_a_end: u8;
    static prop_cpu_b_work: u8;
    static prop_cpu_b_end: u8;
    static prop_cpu_c_work: u8;
    static prop_cpu_c_end: u8;
    static prop_yield_return: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("proportional.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym yield_call, fail = sym fail, yield_time = sym YIELD_TIME,
);

#[repr(C)]
#[derive(Clone, Copy)]
struct Row {
    // The interrupted task's actual return PC, stack frame, counter interval
    // and live progress. Timer deadlines are absolute CNTP_CVAL timestamps.
    pc: u64,
    frame: u64,
    start: u64,
    stop: u64,
    deadline: u64,
    progress: u64,
    normalization: u32,
    pass_start: [u32; 3],
    pass_stop: [u32; 3],
    epoch: u8,
    slot: u8,
    task: u8,
    ready: u8,
    charge: u8,
    ppi: u8,
    draw: u8,
    total: u8,
    low: u8,
    high: u8,
    reserved: [u8; 2],
}

#[repr(C)]
struct Witness {
    count: u64,
    ticks: [u64; 2],
    seeds: [u64; 2],
    join_before: [u32; 3],
    join_after: [u32; 3],
    rows: [Row; ROWS],
}

#[unsafe(no_mangle)]
static mut proportional_witness: Witness = Witness {
    count: 0,
    ticks: [0; 2],
    seeds: SEEDS,
    join_before: [0; 3],
    join_after: [0; 3],
    rows: [Row {
        pc: 0,
        frame: 0,
        start: 0,
        stop: 0,
        deadline: 0,
        progress: 0,
        normalization: 0,
        pass_start: [0; 3],
        pass_stop: [0; 3],
        epoch: 0,
        slot: 0,
        task: 0,
        ready: 0,
        charge: 0,
        ppi: 0,
        draw: 0,
        total: 0,
        low: 0,
        high: 0,
        reserved: [0; 2],
    }; ROWS],
};
const _: () = assert!(size_of::<Row>() == 88);
static YIELD_TIME: AtomicU64 = AtomicU64::new(0);
static mut STACKS: [Stack; 9] = [const { Stack([0; STACK_BYTES]) }; 9];
static mut ACTIVE: Option<Bench> = None;

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"PROPORTIONAL FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

fn disable_timer() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
}

struct Bench {
    policy: Proportional<3>,
    epoch: usize,
    slot: usize,
    current: Option<usize>,
    frames: [*mut Frame; 3],
    progress: [u64; 3],
    served: [u32; 3],
    ticks: [u64; 2],
    start: u64,
    deadline: u64,
    pass_start: [u32; 3],
}

fn with_bench<R>(operation: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    // Only the masked CPU in boot or in one of the exception handlers borrows ACTIVE.
    unsafe { operation((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(epoch: usize, ticks: [u64; 2]) -> Self {
        let policy = if let Some(&seed) = SEEDS.get(epoch) {
            Proportional::lottery(WEIGHTS, seed)
        } else {
            Proportional::stride(WEIGHTS)
        }
        .unwrap_or_else(|| fail());
        Self {
            policy,
            epoch,
            slot: 0,
            current: None,
            frames: [ptr::null_mut(); 3],
            progress: [0; 3],
            served: [0; 3],
            ticks,
            start: 0,
            deadline: 0,
            pass_start: [0; 3],
        }
    }

    fn stride(&self) -> bool {
        self.epoch == SEEDS.len()
    }

    fn top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 3 + id + 1) * STACK_BYTES)
    }

    fn cookie(&self, id: usize) -> u64 {
        0x5100 + (self.epoch * 3 + id) as u64
    }

    fn begin(&mut self) -> *mut Frame {
        let first: unsafe extern "C" fn() = if self.stride() {
            prop_yielder
        } else {
            prop_cpu_a
        };
        let entries: [unsafe extern "C" fn(); 3] = [first, prop_cpu_b, prop_cpu_c];
        for (id, entry) in entries.into_iter().enumerate() {
            // Each epoch starts on separate, unused zero-initialized stacks.
            unsafe {
                let f = self.top(id).sub(size_of::<Frame>()).cast::<Frame>();
                (*f).elr = entry as usize as u64;
                (*f).spsr = 0x3c5;
                (*f).gpr[19] = self.cookie(id);
                (*f).gpr[23] = id as u64;
                self.frames[id] = f;
            }
        }
        require(
            self.policy
                .set_ready_mask(if self.stride() { 3 } else { 7 }),
        );
        if !self.stride() {
            // Checking the inclusive first and last ticket of every interval
            // must leave the generator untouched for the published seed.
            for (id, (lo, hi)) in [(0, 1), (1, 3), (3, 7)].into_iter().enumerate() {
                require(self.policy.winner_for_draw(lo) == Some(id));
                require(self.policy.winner_for_draw(hi - 1) == Some(id));
            }
            require(self.policy.winner_for_draw(7).is_none());
        }
        self.dispatch()
    }

    fn passes(&self) -> [u32; 3] {
        if self.stride() {
            core::array::from_fn(|id| {
                u32::try_from(self.policy.pass(id).unwrap_or_else(|| fail()))
                    .unwrap_or_else(|_| fail())
            })
        } else {
            [0; 3]
        }
    }

    fn tick(&self) -> u64 {
        self.ticks[usize::from(self.stride())]
    }

    fn dispatch(&mut self) -> *mut Frame {
        require(self.current.is_none());
        let id = self.policy.dispatch().unwrap_or_else(|| fail());
        require(self.policy.ready_mask() & (1 << id) != 0);
        self.pass_start = self.passes();
        self.current = Some(id);
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        let tick = self.tick();
        let grant = if self.stride() { 2 } else { 1 };
        self.start = counter();
        self.deadline = self.start + grant * tick;
        YIELD_TIME.store(self.start + tick, Ordering::Release);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) self.deadline, in(reg) 1u64, options(nostack, preserves_flags));
        }
        frame
    }

    fn end_slot(&mut self, frame: *mut Frame, stopped: u64, ppi: u8) -> *mut Frame {
        let id = self.current.take().unwrap_or_else(|| fail());
        require(
            self.frames[id].is_null()
                && frame.addr() == self.top(id).addr() - 16 - size_of::<Frame>(),
        );
        let f = unsafe { &*frame };
        let cookie = self.cookie(id);
        require(f.spsr & 0x8f == 5 && f.gpr[23] == id as u64);
        require(f.gpr[19] == cookie && f.tpidr_el1 == cookie);
        require(unsafe { ptr::read_volatile(self.top(id).sub(16).cast::<u64>()) } == cookie);
        let pc = f.elr;
        let progress = f.gpr[20];
        require(progress > self.progress[id]);
        self.progress[id] = progress;
        self.frames[id] = frame;
        let elapsed = stopped.checked_sub(self.start).unwrap_or_else(|| fail()) / self.tick();
        let charge = if ppi == NONE {
            require(self.stride() && id == 0 && pc == (&raw const prop_yield_return).addr() as u64);
            require(stopped < self.deadline && elapsed == 1);
            elapsed as u32
        } else {
            let grant = if self.stride() { 2 } else { 1 };
            require(stopped >= self.deadline && elapsed >= u64::from(grant));
            let (lo, hi) = match id {
                0 if !self.stride() => (&raw const prop_cpu_a_work, &raw const prop_cpu_a_end),
                1 => (&raw const prop_cpu_b_work, &raw const prop_cpu_b_end),
                2 => (&raw const prop_cpu_c_work, &raw const prop_cpu_c_end),
                _ => fail(),
            };
            require((lo.addr()..hi.addr()).contains(&(pc as usize)));
            grant
        };
        let ready = self.policy.ready_mask() as u8;
        let (draw, total, low, high, normalization) = if self.stride() {
            let offset = self.policy.last_normalization().unwrap_or_else(|| fail());
            (
                NONE,
                0,
                0,
                0,
                u32::try_from(offset).unwrap_or_else(|_| fail()),
            )
        } else {
            let (draw, total) = self.policy.last_draw().unwrap_or_else(|| fail());
            let low: u32 = WEIGHTS[..id].iter().copied().map(u32::from).sum();
            let high = low + u32::from(WEIGHTS[id]);
            require(total == 7 && (low..high).contains(&draw));
            (draw as u8, total as u8, low as u8, high as u8, 0)
        };
        require(self.policy.stop(id, charge));
        let pass_stop = self.passes();
        let row = Row {
            pc,
            frame: frame.addr() as u64,
            start: self.start,
            stop: stopped,
            deadline: self.deadline,
            progress,
            normalization,
            pass_start: self.pass_start,
            pass_stop,
            epoch: self.epoch as u8,
            slot: self.slot as u8,
            task: id as u8,
            ready,
            charge: charge as u8,
            ppi,
            draw,
            total,
            low,
            high,
            reserved: [0; 2],
        };
        unsafe {
            let index = ptr::read_volatile(&raw const proportional_witness.count) as usize;
            require(index < ROWS);
            ptr::write_volatile(&raw mut proportional_witness.rows[index], row);
            ptr::write_volatile(&raw mut proportional_witness.count, index as u64 + 1);
        }
        self.served[id] += charge;
        self.slot += 1;
        if self.stride() {
            self.check_stride(row);
            if self.slot == STRIDE_SLOTS {
                require(self.served == [2, 4, 8]);
                uart_write(b"STRIDE SERVICE JOIN\nPROPORTIONAL PASS\n");
                park()
            }
        } else if self.slot == WINDOW {
            // Compare exact integer fractions. The two independent 32-quantum
            // windows each allow an absolute count deviation of eight quanta.
            for (observed, weight) in self.served.into_iter().zip(WEIGHTS) {
                require(
                    observed > 0
                        && (observed * 7).abs_diff(WINDOW as u32 * u32::from(weight)) <= 8 * 7,
                );
            }
            uart_write(if self.epoch == 0 {
                b"LOTTERY SEED A\n"
            } else {
                b"LOTTERY SEED B\n"
            });
            *self = Self::new(self.epoch + 1, self.ticks);
            return self.begin();
        }
        self.dispatch()
    }

    fn check_stride(&mut self, row: Row) {
        // Verify the policy's choice only after this task actually executed.
        let slot = usize::from(row.slot);
        let expected = if slot < 8 {
            [0, 1][slot % 2]
        } else {
            [0, 1, 2, 2][slot % 4]
        };
        require(row.task == expected && row.ready == if slot < 8 { 3 } else { 7 });
        let mut charged = row.pass_start;
        charged[usize::from(expected)] +=
            u32::from(row.charge) * (SCALE / u32::from(WEIGHTS[usize::from(expected)]));
        require(row.pass_stop == charged);
        if slot == 8 {
            require(row.normalization == SCALE && row.pass_start == [0; 3])
        }
        if self.slot == 8 {
            // Before normalization at the next dispatch, the new entrant must
            // join the current floor. A stale zero would put C ahead of A/B.
            require(self.served == [4, 8, 0] && row.pass_stop == [SCALE, SCALE, 0]);
            require(self.policy.set_ready_mask(7));
            let joined = self.passes();
            require(joined == [SCALE; 3]);
            unsafe {
                ptr::write_volatile(&raw mut proportional_witness.join_before, row.pass_stop);
                ptr::write_volatile(&raw mut proportional_witness.join_after, joined);
            }
            self.served = [0; 3];
        }
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nostack, preserves_flags)) };
    let ticks = [freq / 100, freq / 40];
    require(ticks[0] != 0 && ticks[1] != 0);
    unsafe {
        ptr::write_volatile(&raw mut proportional_witness.ticks, ticks);
        ACTIVE = Some(Bench::new(0, ticks));
        sched_start_frame(with_bench(Bench::begin))
    }
}

extern "C" fn yield_call(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    disable_timer();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && esr & 0xffff == 1);
    with_bench(|b| b.end_slot(frame, stopped, NONE))
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
    disable_timer();
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|b| b.end_slot(frame, stopped, (iar & 0x3ff) as u8))
}
