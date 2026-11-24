//! Hardware-timed CBS grants. An SVC relinquishes the rest of a charged slot.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::temporal_policy::HardCbs;
use super::{park, uart_write};

const IDLE: usize = 2;
const SLOTS: usize = 34;
type Row = [u64; 24];

#[unsafe(no_mangle)]
static mut cbs_wake_witness: [Row; SLOTS] = [[0; 24]; SLOTS];
static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut STATE: Option<Bench> = None;

unsafe extern "C" {
    fn cbs_wake_a();
    fn cbs_wake_b();
    fn cbs_wake_idle();
    static cbs_wake_a_resume: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("cbs_wake.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    irq = sym irq,
    svc = sym block,
    fail = sym fail,
);

struct Bench {
    policy: HardCbs<2>,
    frames: [*mut Frame; 3],
    current: usize,
    epoch: u64,
    quantum: u64,
    blocks: u64,
    sleeping: bool,
    progress: [u64; 3],
    row: Row,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack)) };
    uart_write(b"CBS WAKE FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let value: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack)) };
    value
}

fn top(id: usize) -> *mut u8 {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// Callers hold the exception mask on the single boot CPU.
fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    unsafe { f((*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn take(&mut self, id: usize) -> *mut Frame {
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        self.current = id;
        frame
    }

    fn save(&mut self, frame: *mut Frame) {
        let id = self.current;
        require(self.frames[id].is_null());
        require(frame.addr() == top(id).addr() - 16 - size_of::<Frame>());
        let tag = [0xa19, 0xb19, 0xc19][id];
        let saved = unsafe { &*frame };
        require(saved.gpr[19] == tag && saved.tpidr_el1 == tag);
        require(saved.spsr & 0x8f == 5 && saved.gpr[20] > self.progress[id]);
        require(unsafe { ptr::read_volatile(top(id).sub(16).cast::<u64>()) } == tag);
        self.progress[id] = saved.gpr[20];
        self.frames[id] = frame;
    }

    fn allocations(&mut self, column: usize) {
        for id in 0..2 {
            let (q, d) = self.policy.allocation(id).unwrap_or_else(|| fail());
            self.row[column + 2 * id] = u64::from(q);
            self.row[column + 2 * id + 1] = d;
        }
    }

    fn dispatch(&mut self) -> *mut Frame {
        let t = self.policy.now();
        let demand = match t {
            0 | 4 | 5 | 13 | 22 => 1,
            23 => 100,
            _ => 0,
        };
        self.row = [0; 24];
        self.row[0] = t;
        if demand > 0 {
            require(t == 0 || self.sleeping);
            self.policy.release(0, demand).unwrap_or_else(|_| fail());
            self.sleeping = false;
            self.row[1] = 1;
        }
        if t == 1 {
            self.policy.release(1, 100).unwrap_or_else(|_| fail());
            self.row[1] = 2;
        }
        self.allocations(2);
        let id = self
            .policy
            .tick()
            .unwrap_or_else(|_| fail())
            .unwrap_or(IDLE);
        require(id != 0 || !self.sleeping);
        self.row[6] = id as u64;
        self.allocations(7);
        let deadline = self.epoch + (t + 1) * self.quantum;
        let frame = self.take(id);
        self.row[11..16].copy_from_slice(&[
            self.epoch,
            self.quantum,
            deadline,
            unsafe { (*frame).elr },
            frame.addr() as u64,
        ]);
        require(counter() < deadline);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) deadline, in(reg) 1u64, options(nostack))
        };
        frame
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let entries: [unsafe extern "C" fn(); 3] = [cbs_wake_a, cbs_wake_b, cbs_wake_idle];
    let frames = core::array::from_fn(|id| unsafe {
        let frame = top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*frame).elr = entries[id] as usize as u64;
        (*frame).spsr = 0x3c5;
        frame
    });
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack)) };
    let quantum = frequency / 20;
    let epoch = counter();
    require(quantum > 0 && epoch.checked_add(quantum * (SLOTS as u64 + 1)).is_some());
    unsafe {
        STATE = Some(Bench {
            policy: HardCbs::new(0, [(2, 8), (1, 4)]).unwrap_or_else(|| fail()),
            frames,
            current: IDLE,
            epoch,
            quantum,
            blocks: 0,
            sleeping: false,
            progress: [0; 3],
            row: [0; 24],
        });
        sched_start_frame(with_bench(Bench::dispatch))
    }
}

extern "C" fn block(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack)) };
    require(esr >> 26 == 0x15 && esr & 0xffff == 29);
    with_bench(|b| {
        let stamp = counter();
        require(b.current == 0 && !b.sleeping && b.blocks < 5 && stamp < b.row[13]);
        b.save(frame);
        let saved = unsafe { &*frame };
        require(saved.elr == (&raw const cbs_wake_a_resume).addr() as u64);
        b.blocks += 1;
        require(saved.gpr[21] == b.blocks);
        b.sleeping = true;
        b.row[20..23].copy_from_slice(&[stamp, saved.elr, saved.gpr[20]]);
        b.take(IDLE)
    })
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let stamp = counter();
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    let compare: u64;
    unsafe {
        asm!("mrs {}, cntp_ctl_el0", "mrs {}, cntp_cval_el0", out(reg) ctl, out(reg) compare,
            options(nostack));
    }
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    unsafe { asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack)) };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack)) };
    with_bench(|b| {
        require(compare == b.row[13] && (compare..compare + b.quantum).contains(&stamp));
        b.save(frame);
        let saved = unsafe { &*frame };
        b.row[16..20].copy_from_slice(&[stamp, b.current as u64, saved.elr, saved.gpr[20]]);
        b.row[23] = frame.addr() as u64;
        let index = b.row[0] as usize;
        unsafe { ptr::write_volatile((&raw mut cbs_wake_witness).cast::<Row>().add(index), b.row) };
        if index + 1 == SLOTS {
            require(b.blocks == 5 && !b.sleeping && b.progress.iter().all(|&p| p > 0));
            uart_write(b"CBS WAKE TIMER STOP\nCBS WAKE PASS\n");
            park()
        }
        b.dispatch()
    })
}
