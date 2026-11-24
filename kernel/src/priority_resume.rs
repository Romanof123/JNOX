//! Single-CPU fixed-priority FIFO and measured residual-RR experiment.
use super::context::*;
use super::ready_queue::ReadyQueue;
use super::{park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

const HIGH: usize = 0;
const B: usize = 1;
const A: usize = 2;
const NONE: usize = 3;
const BOOT: u64 = 0;
const ARRIVE_B: u64 = 1;
const ARRIVE_HIGH: u64 = 2;
const YIELD: u64 = 3;
const EXIT: u64 = 4;
const EXPIRY: u64 = 5;
const RR: usize = 2;
const DEFERRED: usize = 1;

#[repr(C)]
struct Witness {
    count: u64,
    frequency: u64,
    quantum: u64,
    rows: [[u64; 18]; 32],
}

#[unsafe(no_mangle)]
static mut priority_resume_witness: Witness = unsafe { core::mem::zeroed() };
static GATE: AtomicU64 = AtomicU64::new(0);
static mut STACKS: [[Stack; 3]; 3] = [const { [const { Stack([0; STACK_BYTES]) }; 3] }; 3];
static mut STATE: Option<State> = None;

unsafe extern "C" {
    fn priority_resume_a_entry();
    fn priority_resume_b_entry();
    fn priority_resume_high_entry();
    static priority_resume_a_work: u8;
    static priority_resume_after_yield: u8;
    static priority_resume_a_after_exit: u8;
    static priority_resume_b_after_exit: u8;
    static priority_resume_high_after_exit: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("priority_resume.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    gate = sym GATE, irq = sym interrupt, svc = sym syscall, fail = sym fail,
);

fn counter() -> u64 {
    let now: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) };
    now
}

fn require(condition: bool) {
    if !condition {
        fail()
    }
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"PRIORITY RESUME FAIL\n");
    park()
}

struct State {
    epoch: usize,
    q: u64,
    current: usize,
    queue: ReadyQueue<3>,
    frames: [*mut Frame; 3],
    budget: [u64; 3],
    begin: u64,
    due: u64,
    arrivals: usize,
    exits: usize,
}

fn priority(id: usize) -> u8 {
    if id == HIGH { 2 } else { 1 }
}

impl State {
    fn new(epoch: usize, q: u64) -> Self {
        GATE.store(0, Ordering::Release);
        Self {
            epoch,
            q,
            current: NONE,
            queue: ReadyQueue::new(),
            frames: [ptr::null_mut(); 3],
            budget: [if epoch == RR { q } else { 0 }; 3],
            begin: 0,
            due: 0,
            arrivals: 0,
            exits: 0,
        }
    }

    fn top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 3 + id + 1) * STACK_BYTES)
    }

    fn cookie(&self, id: usize) -> u64 {
        0xa900 + (self.epoch * 3 + id) as u64
    }

    fn arrive(&mut self, id: usize) {
        let entry = [
            priority_resume_high_entry,
            priority_resume_b_entry,
            priority_resume_a_entry,
        ][id];
        unsafe {
            let frame = self.top(id).sub(size_of::<Frame>()).cast::<Frame>();
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = 0x3c5;
            (*frame).gpr[19] = self.cookie(id);
            (*frame).gpr[21] = u64::from(self.epoch == RR);
            self.frames[id] = frame;
        }
        require(self.queue.push_back(id));
    }

    fn check(&self, frame: *mut Frame, event: u64) {
        let id = self.current;
        let cookie = self.cookie(id);
        require(frame.addr() + size_of::<Frame>() + 16 == self.top(id).addr());
        let saved = unsafe { &*frame };
        let word = unsafe { ptr::read_volatile(self.top(id).sub(16).cast::<u64>()) };
        require(saved.gpr[19] == cookie && word == cookie && saved.simd[31] == [cookie; 2]);
        require(saved.tpidr_el1 == cookie && saved.spsr & 0x8f == 5 && saved.gpr[20] > 0);
        let pc = saved.elr as usize;
        match event {
            ARRIVE_B | ARRIVE_HIGH | EXPIRY => {
                let start = (&raw const priority_resume_a_work).addr();
                require(id == A && (start..start + 12).contains(&pc));
            }
            YIELD => require(id == A && pc == (&raw const priority_resume_after_yield).addr()),
            EXIT => {
                let site = [
                    &raw const priority_resume_high_after_exit,
                    &raw const priority_resume_b_after_exit,
                    &raw const priority_resume_a_after_exit,
                ][id];
                require(pc == site.addr());
                require(id == A || saved.gpr[20] == 4097);
            }
            _ => fail(),
        }
    }

    fn dispatch(&mut self, event: u64, frame: *mut Frame, end: u64) -> *mut Frame {
        let from = self.current;
        let mut row = [0u64; 18];
        row[0] = self.epoch as u64;
        row[1] = event;
        row[2] = from as u64;
        if from != NONE {
            self.check(frame, event);
            self.frames[from] = frame;
            row[5] = self.begin;
            row[6] = end;
            row[7] = self.due;
            row[8] = self.budget[from];
            require(end > self.begin);
            if self.epoch == RR {
                self.budget[from] = self.budget[from].saturating_sub(end - self.begin);
                require((self.budget[from] == 0) == (event == EXPIRY));
            }
            row[9] = self.budget[from];
            row[13] = unsafe { (*frame).elr };
            row[15] = frame.addr() as u64;
            row[17] = unsafe { (*frame).gpr[20] };
        }
        let mut retain = false;
        match event {
            BOOT => self.arrive(A),
            ARRIVE_B | ARRIVE_HIGH => {
                let arriving = if event == ARRIVE_B { B } else { HIGH };
                self.arrive(arriving);
                self.arrivals += 1;
                retain = self.epoch == DEFERRED || priority(arriving) <= priority(from);
                if !retain {
                    require(self.queue.push_front(from));
                }
                if event == ARRIVE_HIGH && self.epoch != RR {
                    GATE.store(1, Ordering::Release);
                }
            }
            YIELD | EXPIRY => {
                require(self.queue.push_back(from));
                if event == EXPIRY {
                    self.budget[from] = self.q;
                    GATE.store(1, Ordering::Release);
                }
            }
            EXIT => {
                self.exits += 1;
                self.frames[from] = ptr::null_mut();
            }
            _ => fail(),
        }
        let to = if retain {
            from
        } else {
            self.queue.pop_highest(priority).unwrap_or(NONE)
        };
        self.current = to;
        row[3] = to as u64;
        // Read the actual queue through the bounded visitor without removing a job.
        let snapshot = core::cell::Cell::new((0u64, 0u32));
        self.queue.pop_ranked(
            |id| {
                let (bits, shift) = snapshot.get();
                snapshot.set((bits | (((id + 1) as u64) << shift), shift + 4));
                None::<u8>
            },
            |a, b| a > b,
        );
        row[4] = snapshot.get().0;
        let mut next = ptr::null_mut();
        if to != NONE {
            next = core::mem::replace(&mut self.frames[to], ptr::null_mut());
            require(!next.is_null());
            row[14] = unsafe { (*next).elr };
            row[16] = next.addr() as u64;
            row[12] = self.budget[to];
            let delay = if self.arrivals < 2 {
                self.q / 4
            } else {
                self.budget[to]
            };
            self.begin = counter();
            self.due = if delay == 0 { 0 } else { self.begin + delay };
            if delay != 0 {
                unsafe {
                    asm!("msr cntp_cval_el0, {}", "mov x9, #1", "msr cntp_ctl_el0, x9", "isb",
                    in(reg) self.due, out("x9") _, options(nostack, preserves_flags))
                };
            }
            row[10] = self.begin;
            row[11] = self.due;
        } else {
            require(self.exits == 3 && self.arrivals == 2);
        }
        unsafe {
            let index = priority_resume_witness.count as usize;
            require(index < 32);
            ptr::write_volatile(&raw mut priority_resume_witness.rows[index], row);
            ptr::write_volatile(&raw mut priority_resume_witness.count, index as u64 + 1);
        }
        next
    }
}

fn disable() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
}

// The only callers are boot and masked uniprocessor exception handlers.
fn state() -> &'static mut State {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail()) }
}

extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let end = counter();
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    disable();
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    let s = state();
    require(s.due > 0 && end >= s.due && end - s.due <= s.q / 8);
    let event = match s.arrivals {
        0 => ARRIVE_B,
        1 => ARRIVE_HIGH,
        _ => EXPIRY,
    };
    s.dispatch(event, frame, end)
}

extern "C" fn syscall(frame: *mut Frame) -> *mut Frame {
    let end = counter();
    disable();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15);
    let event = match esr & 0x01ff_ffff {
        1 => EXIT,
        2 => YIELD,
        _ => fail(),
    };
    let s = state();
    let next = s.dispatch(event, frame, end);
    if !next.is_null() {
        return next;
    }
    if s.epoch == RR {
        uart_write(b"PRIORITY RESUME PASS\n");
        park()
    }
    *s = State::new(s.epoch + 1, s.q);
    s.dispatch(BOOT, ptr::null_mut(), 0)
}

pub(super) fn run() -> ! {
    init_timer_irq();
    uart_write(b"PRIORITY RESUME BEGIN\n");
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let q = frequency / 10;
    require(q >= 64);
    unsafe {
        ptr::write_volatile(&raw mut priority_resume_witness.frequency, frequency);
        ptr::write_volatile(&raw mut priority_resume_witness.quantum, q);
        STATE = Some(State::new(0, q));
        sched_start_frame(state().dispatch(BOOT, ptr::null_mut(), 0))
    }
}
