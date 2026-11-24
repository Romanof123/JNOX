//! Two independent admission orders on the ordinary uniprocessor SVC path.
use super::{
    context::*,
    mutex_policy::{Acquire, LockSet, LockSpec, Protocol},
    park,
    ready_queue::ReadyQueue,
    uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
};

global_asm!(".set JNOX_AP,1", include_str!("context.S"),
    frame_size=const size_of::<Frame>(), elr=const offset_of!(Frame,elr),
    fpcr=const offset_of!(Frame,fpcr), tpidr_el1=const offset_of!(Frame,tpidr_el1),
    tpidr_el0=const offset_of!(Frame,tpidr_el0), simd=const offset_of!(Frame,simd),
    irq=sym unexpected, svc=sym request, fail=sym unexpected);

const NA: u64 = u64::MAX;
const PRI: [u8; 4] = [2, 7, 7, 0];
const IDLE: usize = 3;
const X: usize = 0;
const LOCK: u64 = 1;
const UNLOCK: u64 = 2;
const ACTIVATE: u64 = 3;
const EXIT: u64 = 4;
const SEED: u64 = 5;
const OBSERVE: u64 = 6;
const START: u64 = 7;
const ROWS: usize = 40;
static mut STACKS: [[Stack; 4]; 2] = [const { [const { Stack([0; STACK_BYTES]) }; 4] }; 2];
#[unsafe(export_name = "jnox_ap_returns")]
static mut RETURNS: [u64; 193] = [0; 193];
static mut JOURNAL: [[u64; 23]; ROWS] = [[0; 23]; ROWS];
static mut USED: usize = 0;
static mut ACTIVE: Option<Runtime> = None;
static mut PROTECTED: [u64; 2] = [0; 2];

fn svc(op: u64, arg: u64, observation: [u64; 3]) -> u64 {
    let mut result = arg;
    unsafe {
        asm!("svc #0", in("x8") op, inout("x0") result,
        in("x1") observation[0], in("x2") observation[1], in("x3") observation[2],
        options(nostack))
    }
    result
}

fn checked(op: u64, arg: u64) {
    if svc(op, arg, [NA; 3]) != 0 {
        stop(1)
    }
}

fn exit() -> ! {
    checked(EXIT, 0);
    stop(1)
}

extern "C" fn owner() -> ! {
    checked(LOCK, 0);
    unsafe { ptr::write_volatile(&raw mut PROTECTED, [0, 4]) };
    checked(SEED, 0);
    let first = unsafe { (*(&raw const ACTIVE)).as_ref().unwrap().epoch as u64 };
    checked(ACTIVATE, first);
    checked(ACTIVATE, 3 - first);
    checked(UNLOCK, 0);
    exit()
}

#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn fifo_worker_acquire() -> u64 {
    svc(LOCK, 0, [NA; 3])
}

fn worker(digit: u64) -> ! {
    let result = fifo_worker_acquire();
    if result != 0 {
        stop(2)
    }
    let [count, value] = unsafe { ptr::read_volatile(&raw const PROTECTED) };
    unsafe { ptr::write_volatile(&raw mut PROTECTED, [count + 1, 10 * value + digit]) };
    if svc(OBSERVE, 0, [result, count, value]) != 0 {
        stop(1)
    }
    checked(UNLOCK, 0);
    exit()
}

extern "C" fn worker_a() -> ! {
    worker(1)
}
extern "C" fn worker_b() -> ! {
    worker(2)
}
extern "C" fn idle() -> ! {
    let r = unsafe { (*(&raw const ACTIVE)).as_ref().unwrap() };
    if r.done != 7
        || r.current != IDLE
        || r.locks.owner(X).is_some()
        || (0..4).any(|id| r.locks.waiting(id).is_some() || r.locks.effective(id) != Some(PRI[id]))
    {
        stop(3)
    }
    if r.epoch == 1 {
        start(2)
    }
    stop(0)
}
const ENTRIES: [extern "C" fn() -> !; 4] = [owner, worker_a, worker_b, idle];

fn top(epoch: usize, id: usize) -> usize {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add(((epoch - 1) * 4 + id + 1) * STACK_BYTES)
        .addr()
}

struct Runtime {
    epoch: usize,
    locks: LockSet<4, 1>,
    ready: ReadyQueue<4>,
    frames: [*mut Frame; 4],
    current: usize,
    admitted: u8,
    done: u8,
}

impl Runtime {
    fn owned(&self, id: usize, frame: *mut Frame) -> bool {
        let hi = top(self.epoch, id);
        frame.addr() % 16 == 0
            && (hi - STACK_BYTES..=hi - size_of::<Frame>()).contains(&frame.addr())
    }

    fn enqueue(&mut self, id: usize, front: bool) {
        if self.done & (1 << id) != 0
            || self.locks.waiting(id).is_some()
            || !self.owned(id, self.frames[id])
            || !(if front {
                self.ready.push_front(id)
            } else {
                self.ready.push_back(id)
            })
        {
            stop(4)
        }
    }

    fn choose(&mut self) -> *mut Frame {
        self.current = self
            .ready
            .pop_highest(|i| self.locks.effective(i).unwrap_or(0))
            .unwrap_or_else(|| stop(4));
        let frame = core::mem::replace(&mut self.frames[self.current], ptr::null_mut());
        if !self.owned(self.current, frame) {
            stop(4)
        }
        frame
    }

    fn record(
        &self,
        op: u64,
        arg: u64,
        actor: u64,
        saved: *mut Frame,
        saved_x0: u64,
        selected: *mut Frame,
        grant: Option<usize>,
        observed: [u64; 3],
    ) {
        let mut runnable = 1u64 << self.current;
        for &id in self.ready.entries() {
            runnable |= 1 << id
        }
        let mut waits = 0;
        let mut priorities = 0;
        for id in 0..4 {
            if self.locks.waiting(id) == Some(X) {
                waits |= 1 << id
            }
            priorities |= u64::from(self.locks.effective(id).unwrap()) << (8 * id);
        }
        unsafe {
            if USED >= ROWS {
                stop(5)
            }
            let (addr, pc) = if saved.is_null() {
                (NA, NA)
            } else {
                (saved.addr() as u64, (*saved).elr)
            };
            let [count, value] = ptr::read_volatile(&raw const PROTECTED);
            JOURNAL[USED] = [
                self.epoch as u64,
                USED as u64 + 1,
                op,
                arg,
                actor,
                self.current as u64,
                addr,
                pc,
                selected.addr() as u64,
                (*selected).elr,
                (*selected).gpr[0],
                runnable,
                waits,
                priorities,
                self.locks.owner(X).map_or(NA, |i| i as u64),
                self.done as u64,
                grant.map_or(NA, |i| i as u64),
                count,
                value,
                saved_x0,
                observed[0],
                observed[1],
                observed[2],
            ];
            USED += 1;
        }
    }
}

extern "C" fn request(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1", out(reg) esr, options(nostack)) }
    if esr >> 26 != 0x15 || esr & 0xffff != 0 {
        stop(6)
    }
    let r = unsafe { (*(&raw mut ACTIVE)).as_mut().unwrap() };
    let id = r.current;
    if !r.owned(id, frame)
        || !r.frames[id].is_null()
        || unsafe { (*frame).tpidr_el1 } != id as u64 + 1
    {
        stop(4)
    }
    r.frames[id] = frame;
    let op = unsafe { (*frame).gpr[8] };
    let arg = unsafe { (*frame).gpr[0] };
    let mut observed = [NA; 3];
    let mut grant = None;
    match op {
        LOCK if arg == 0 => match r.locks.acquire(id, X).unwrap_or_else(|| stop(7)) {
            Acquire::Held => {
                unsafe { (*frame).gpr[0] = 0 };
                r.enqueue(id, true)
            }
            Acquire::Blocked => {}
        },
        UNLOCK if arg == 0 => {
            grant = r.locks.release(id, X).unwrap_or_else(|| stop(7));
            if let Some(t) = grant {
                if !r.owned(t, r.frames[t]) {
                    stop(4)
                }
                unsafe { (*r.frames[t]).gpr[0] = 0 };
                r.enqueue(t, false);
            }
            unsafe { (*frame).gpr[0] = 0 };
            r.enqueue(id, true)
        }
        ACTIVATE if id == 0 && (1..=2).contains(&arg) => {
            let peer = arg as usize;
            if r.admitted & (1 << peer) != 0 || r.locks.owner(X) != Some(id) {
                stop(8)
            }
            r.admitted |= 1 << peer;
            r.enqueue(peer, false);
            unsafe { (*frame).gpr[0] = 0 };
            r.enqueue(id, false)
        }
        EXIT if id != IDLE && r.locks.owner(X) != Some(id) => r.done |= 1 << id,
        SEED | OBSERVE if r.locks.owner(X) == Some(id) => {
            if op == OBSERVE {
                observed = unsafe { [(*frame).gpr[1], (*frame).gpr[2], (*frame).gpr[3]] };
            }
            unsafe { (*frame).gpr[0] = 0 };
            r.enqueue(id, true)
        }
        _ => stop(8),
    }
    let next = r.choose();
    r.record(op, arg, id as u64, frame, arg, next, grant, observed);
    next
}

fn start(epoch: usize) -> ! {
    unsafe { asm!("msr daifset,#15", options(nostack)) };
    let mut r = Runtime {
        epoch,
        locks: LockSet::new(
            PRI,
            [LockSpec {
                users: 7,
                protocol: Protocol::Inherit,
            }],
        )
        .unwrap(),
        ready: ReadyQueue::new(),
        frames: [ptr::null_mut(); 4],
        current: IDLE,
        admitted: 0,
        done: 0,
    };
    for (id, entry) in ENTRIES.into_iter().enumerate() {
        // Each bank is zeroed BSS used once. The previous idle stack is live.
        let frame = (top(epoch, id) - size_of::<Frame>()) as *mut Frame;
        unsafe {
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = 0x345;
            (*frame).tpidr_el1 = id as u64 + 1;
        }
        r.frames[id] = frame;
    }
    unsafe { ptr::write_volatile(&raw mut PROTECTED, [0; 2]) };
    r.enqueue(IDLE, false);
    r.enqueue(0, false);
    let next = r.choose();
    r.record(START, 0, NA, ptr::null_mut(), NA, next, None, [NA; 3]);
    unsafe {
        ptr::write(&raw mut ACTIVE, Some(r));
        sched_start_frame(next)
    }
}

fn line(name: &[u8], fields: &[u64]) {
    uart_write(name);
    for &v in fields {
        let mut digits = [0; 16];
        for (i, b) in digits.iter_mut().enumerate() {
            *b = b"0123456789abcdef"[((v >> (60 - 4 * i)) & 15) as usize]
        }
        uart_write(b" ");
        uart_write(&digits)
    }
    uart_write(b"\n")
}

extern "C" fn unexpected() -> ! {
    stop(9)
}

fn stop(reason: u64) -> ! {
    unsafe { asm!("msr daifset,#15", options(nostack)) };
    uart_write(b"FIFO RAW 1\n");
    unsafe {
        let used = USED.min(ROWS);
        let returned = ptr::read_volatile((&raw const RETURNS).cast::<u64>()).min(64);
        line(b"H", &[reason, used as u64, returned]);
        for epoch in 1..=2 {
            for (id, entry) in ENTRIES.into_iter().enumerate() {
                let hi = top(epoch, id);
                line(
                    b"S",
                    &[
                        epoch as u64,
                        id as u64,
                        (hi - STACK_BYTES) as u64,
                        hi as u64,
                        entry as usize as u64,
                    ],
                )
            }
        }
        for i in 0..used {
            line(b"J", &JOURNAL[i])
        }
        for i in 0..returned as usize {
            let base = 1 + 3 * i;
            line(
                b"R",
                &[
                    i as u64 + 1,
                    RETURNS[base],
                    RETURNS[base + 1],
                    RETURNS[base + 2],
                ],
            )
        }
    }
    uart_write(b"FIFO END\n");
    park()
}

pub(super) fn run() -> ! {
    init_timer_irq();
    start(1)
}
