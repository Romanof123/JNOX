//! One boot, one lock protocol. Only masked exception handlers own the scheduler.
use super::{
    context::*,
    mutex_policy::{Acquire, LockSet},
    park,
    ready_queue::ReadyQueue,
    uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
    sync::atomic::{AtomicU64, Ordering::SeqCst},
};

global_asm!(".set JNOX_AP,1", include_str!("context.S"),
    frame_size=const size_of::<Frame>(), elr=const offset_of!(Frame,elr),
    fpcr=const offset_of!(Frame,fpcr), tpidr_el1=const offset_of!(Frame,tpidr_el1),
    tpidr_el0=const offset_of!(Frame,tpidr_el0), simd=const offset_of!(Frame,simd),
    irq=sym interrupt, svc=sym request, fail=sym unexpected);

use super::mutex_policy::{LockSpec, Protocol};
const BASE: usize = 0;
const LOW: usize = 1;
const MED: usize = 2;
const HIGH: usize = 3;
const PRI: [u8; 4] = [1, 2, 3, 4];
const X: usize = 0;
const Y: usize = 1;
const NA: u64 = u64::MAX;
const LOCK: u64 = 1;
const UNLOCK: u64 = 2;
const TIMED: u64 = 3;
const EXIT: u64 = 4;
const IRQ: u64 = 5;
const ROWS: usize = 32;
static mut STACKS: [Stack; 4] = [const { Stack([0; STACK_BYTES]) }; 4];
#[unsafe(export_name = "jnox_ap_returns")]
static mut RETURNS: [u64; 193] = [0; 193];
static mut JOURNAL: [[u64; 28]; ROWS] = [[0; 28]; ROWS];

static mut USED: usize = 0;
static mut ACTIVE: Option<Runtime> = None;
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static IN_WORK: AtomicU64 = AtomicU64::new(0);
static INPUT: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
static mut BATCH: [u64; 2] = [0; 2];
static mut PUBLISHED: [u64; 2] = [0; 2];
static mut OUTCOME: u64 = 0;
static mut MED_SUM: u64 = 0;
static mut TIMER: [u64; 6] = [0; 6];
fn now() -> u64 {
    let t;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)t,options(nostack)) };
    t
}
fn call(op: u64, arg: usize) -> u64 {
    let mut value = arg as u64;
    unsafe { asm!("svc #0",in("x8") op,inout("x0") value,options(nostack)) };
    value
}
fn checked(op: u64, arg: usize) {
    if call(op, arg) != 0 {
        stop(11)
    }
}
fn exit() -> ! {
    checked(EXIT, 0);
    stop(12)
}

extern "C" fn base() -> ! {
    checked(LOCK, Y);
    let mut sum = 0;
    IN_WORK.store(1, SeqCst);
    for chunk in 1..=16384 {
        for n in 0..256 {
            sum += unsafe { ptr::read_volatile(INPUT.as_ptr().add(n & 15)) } as u64;
        }
        CHUNKS.store(chunk, SeqCst);
    }
    unsafe { ptr::write_volatile(&raw mut BATCH, [4194304, sum]) };
    IN_WORK.store(0, SeqCst);
    checked(UNLOCK, Y);
    exit()
}
extern "C" fn low() -> ! {
    checked(LOCK, X);
    checked(LOCK, Y);
    let data = unsafe { ptr::read_volatile(&raw const BATCH) };
    unsafe { ptr::write_volatile(&raw mut PUBLISHED, data) };
    checked(UNLOCK, Y);
    checked(UNLOCK, X);
    exit()
}
extern "C" fn high() -> ! {
    let result = call(TIMED, X);
    unsafe { ptr::write_volatile(&raw mut OUTCOME, result) };
    if result != 1 {
        stop(11)
    }
    exit()
}
extern "C" fn medium() -> ! {
    let sum = (0..256)
        .map(|i| unsafe { ptr::read_volatile(INPUT.as_ptr().add(i & 15)) } as u64)
        .sum();
    unsafe { ptr::write_volatile(&raw mut MED_SUM, sum) };
    exit()
}
struct Runtime {
    locks: LockSet<4, 2>,
    ready: ReadyQueue<4>,
    frames: [*mut Frame; 4],
    current: usize,
    done: u8,
    armed: bool,
    deadline: u64,
    priorities: [u8; 4],
    freq: u64,
}
fn top(id: usize) -> usize {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
        .addr()
}
fn owned_frame(id: usize, f: *mut Frame) -> bool {
    let hi = top(id);
    let a = f.addr();
    a % 16 == 0 && a >= hi - STACK_BYTES && a <= hi - size_of::<Frame>()
}
fn rt() -> &'static mut Runtime {
    unsafe { (*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| stop(1)) }
}
fn tag(id: Option<usize>) -> u64 {
    id.map_or(NA, |t| t as u64)
}
impl Runtime {
    fn enqueue(&mut self, id: usize, front: bool) {
        if self.done & (1 << id) != 0
            || self.locks.waiting(id).is_some()
            || self.frames[id].is_null()
            || !(if front {
                self.ready.push_front(id)
            } else {
                self.ready.push_back(id)
            })
        {
            stop(2)
        }
    }
    fn choose(&mut self) -> *mut Frame {
        self.priorities =
            core::array::from_fn(|id| self.locks.effective(id).unwrap_or_else(|| stop(3)));
        self.current = self
            .ready
            .pop_highest(|i| self.priorities[i])
            .unwrap_or_else(|| stop(3));
        let frame = core::mem::replace(&mut self.frames[self.current], ptr::null_mut());
        if !owned_frame(self.current, frame) {
            stop(4)
        }
        frame
    }
    fn save(&mut self, f: *mut Frame) -> usize {
        let id = self.current;
        if !owned_frame(id, f)
            || !self.frames[id].is_null()
            || unsafe { (*f).tpidr_el1 } != id as u64 + 1
        {
            stop(4)
        }
        self.frames[id] = f;
        id
    }
    fn record(&self, op: u64, arg: usize, id: usize, f: *mut Frame, next: *mut Frame, result: u64) {
        let mut q = 1u64 << self.current;
        for &t in self.ready.entries() {
            q |= 1 << t
        }
        let mut waits = [0u64; 2];
        let mut priorities = 0u64;
        for t in 0..4 {
            if let Some(l) = self.locks.waiting(t) {
                waits[l] |= 1 << t
            }
            priorities |= u64::from(self.priorities[t]) << (8 * t)
        }
        unsafe {
            if USED == ROWS {
                stop(5)
            }
            JOURNAL[USED] = [
                (*f).reserved,
                now(),
                op,
                arg as u64,
                id as u64,
                self.current as u64,
                f.addr() as u64,
                (*f).elr,
                (*f).spsr,
                (*f).gpr[19],
                next.addr() as u64,
                (*next).elr,
                (*next).gpr[19],
                q,
                tag(self.locks.owner(X)),
                tag(self.locks.owner(Y)),
                waits[0],
                waits[1],
                priorities,
                result,
                CHUNKS.load(SeqCst),
                IN_WORK.load(SeqCst),
                self.done as u64,
                (*f).tpidr_el1,
                (*next).tpidr_el1,
                (*next).gpr[0],
                self.ready
                    .entries()
                    .iter()
                    .fold(0u64, |order, &t| order * 16 + t as u64 + 1),
                self.locks
                    .wait_order()
                    .iter()
                    .fold(0u64, |order, &t| order * 16 + u64::from(t) + 1),
            ];
            USED += 1
        }
    }
}

extern "C" fn request(f: *mut Frame) -> *mut Frame {
    let r = rt();
    let id = r.save(f);
    let op = unsafe { (*f).gpr[8] };
    let arg = unsafe { (*f).gpr[0] } as usize;
    let mut result = 0;
    match op {
        LOCK | TIMED => match r.locks.acquire(id, arg) {
            Some(Acquire::Held) => {
                if op == TIMED {
                    stop(6)
                }
                unsafe { (*f).gpr[0] = 0 };
                r.enqueue(id, true);
                if id == BASE {
                    r.enqueue(LOW, false)
                }
            }
            Some(Acquire::Blocked) => {
                result = NA;
                if id == LOW && arg == Y {
                    r.enqueue(HIGH, false);
                    r.enqueue(MED, false);
                } else if id == HIGH && op == TIMED && arg == X {
                    let start = now();
                    r.deadline = start.checked_add(r.freq / 1000).unwrap_or_else(|| stop(6));
                    r.armed = true;
                    unsafe {
                        TIMER[0] = r.freq;
                        TIMER[1] = start;
                        TIMER[2] = r.deadline;
                        asm!("msr cntp_cval_el0, {0}","msr cntp_ctl_el0, {1}","isb",in(reg)r.deadline,in(reg)1u64,options(nostack));
                    }
                } else {
                    stop(6)
                }
            }
            None => stop(6),
        },
        UNLOCK => {
            if let Some(t) = r.locks.release(id, arg).unwrap_or_else(|| stop(7)) {
                let waiter = r.frames[t];
                if !owned_frame(t, waiter) {
                    stop(4)
                }
                unsafe { (*waiter).gpr[0] = 0 };
                r.enqueue(t, false);
            }
            unsafe { (*f).gpr[0] = 0 };
            r.enqueue(id, true)
        }
        EXIT => {
            r.done |= 1 << id;
            if r.done == 15 {
                stop(0)
            }
        }
        _ => stop(8),
    }
    let next = r.choose();
    r.record(op, arg, id, f, next, result);
    next
}
extern "C" fn interrupt(f: *mut Frame) -> *mut Frame {
    let r = rt();
    let id = r.save(f);
    let iar = gic_read(GICC + 0xc);
    let (ctl, cval): (u64, u64);
    unsafe {
        asm!("mrs {},cntp_ctl_el0","mrs {},cntp_cval_el0","msr cntp_ctl_el0,xzr","isb",out(reg)ctl,out(reg)cval,options(nostack));
        TIMER[3] = iar as u64;
        TIMER[4] = ctl;
        TIMER[5] = cval;
    }
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", "isb", options(nostack)) }
    if !r.armed
        || id != BASE
        || iar & 1023 != TIMER_IRQ
        || ctl & 7 != 5
        || cval != r.deadline
        || unsafe { (*f).reserved } < cval
        || !(1..16384).contains(&CHUNKS.load(SeqCst))
        || IN_WORK.load(SeqCst) != 1
        || !r.locks.timeout(HIGH)
    {
        stop(9)
    }
    r.armed = false;
    let waiter = r.frames[HIGH];
    if !owned_frame(HIGH, waiter) {
        stop(4)
    }
    unsafe { (*waiter).gpr[0] = 1 };
    r.enqueue(id, true);
    r.enqueue(HIGH, false);
    let next = r.choose();
    r.record(IRQ, X, id, f, next, 1);
    next
}
fn line(name: &[u8], fields: &[u64]) {
    uart_write(name);
    for &v in fields {
        let mut digits = [0u8; 16];
        for (i, b) in digits.iter_mut().enumerate() {
            *b = b"0123456789abcdef"[((v >> (60 - 4 * i)) & 15) as usize]
        }
        uart_write(b" ");
        uart_write(&digits)
    }
    uart_write(b"\n")
}
extern "C" fn unexpected() -> ! {
    stop(17)
}
fn stop(reason: u64) -> ! {
    unsafe {
        asm!(
            "msr daifset,#0xf",
            "msr cntp_ctl_el0,xzr",
            "isb",
            options(nostack)
        )
    }

    uart_write(b"CHAIN RAW 1\n");
    unsafe {
        line(b"H", &[reason, USED as u64, RETURNS[0]]);
        line(b"T", &*(&raw const TIMER));
        for id in 0..4 {
            line(
                b"S",
                &[
                    id as u64,
                    (top(id) - STACK_BYTES) as u64,
                    top(id) as u64,
                    size_of::<Frame>() as u64,
                ],
            );
        }
        for i in 0..USED {
            line(b"E", &*(&raw const JOURNAL[i]))
        }
        for i in 0..RETURNS[0].min(64) as usize {
            line(b"R", &(&*(&raw const RETURNS))[1 + 3 * i..4 + 3 * i])
        }
        line(
            b"P",
            &[
                BATCH[0],
                BATCH[1],
                PUBLISHED[0],
                PUBLISHED[1],
                OUTCOME,
                MED_SUM,
            ],
        );
    }
    uart_write(b"CHAIN END\n");
    park()
}
pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)freq,options(nostack)) }
    if freq < 1000 || freq % 1000 != 0 {
        stop(13)
    }
    let mut r = Runtime {
        locks: LockSet::new(
            PRI,
            [
                LockSpec {
                    users: 10,
                    protocol: Protocol::Inherit,
                },
                LockSpec {
                    users: 3,
                    protocol: Protocol::Inherit,
                },
            ],
        )
        .unwrap_or_else(|| stop(1)),
        ready: ReadyQueue::new(),
        frames: [ptr::null_mut(); 4],
        current: BASE,
        done: 0,
        armed: false,
        deadline: 0,
        priorities: PRI,
        freq,
    };
    let entries: [extern "C" fn() -> !; 4] = [base, low, medium, high];
    for (id, entry) in entries.into_iter().enumerate() {
        let frame = (top(id) - size_of::<Frame>()) as *mut Frame;
        unsafe {
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = 0x345;
            (*frame).tpidr_el1 = id as u64 + 1;
            (*frame).gpr[19] = 0x6700 + id as u64
        };
        r.frames[id] = frame
    }
    r.enqueue(BASE, false);
    let f = r.choose();
    unsafe {
        ptr::write(&raw mut ACTIVE, Some(r));
        sched_start_frame(f)
    }
}
