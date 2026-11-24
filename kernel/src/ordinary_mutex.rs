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

const CEILING: bool = cfg!(jnox_mutex_ceiling);
const LOW: usize = 0;
const MED: usize = 1;
const HIGH: usize = 2;
const IDLE: usize = 3;
use super::ordinary_mutex_manifest::{PRI, locks};
const X: usize = 0;
const Y: usize = 1;
const NA: u64 = u64::MAX;
const LOCK: u64 = 1;
const UNLOCK: u64 = 2;
const YIELD: u64 = 3;
const EXIT: u64 = 4;
const IRQ: u64 = 5;
const ROWS: usize = 32;
static mut STACKS: [Stack; 4] = [const { Stack([0; STACK_BYTES]) }; 4];
#[unsafe(export_name = "jnox_ap_returns")]
static mut RETURNS: [u64; 193] = [0; 193];
static mut JOURNAL: [[u64; 26]; ROWS] = [[0; 26]; ROWS];
static mut USED: usize = 0;
static mut TIMER: [u64; 8] = [0; 8];
static mut ACTIVE: Option<Runtime> = None;
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static IN_WORK: AtomicU64 = AtomicU64::new(0);
static mut LEDGER: [[u64; 2]; 256] = [[0; 2]; 256];
// Single CPU: ordinary data records have a single writer under their lock.
// The result and timestamps have one task writer per slot and are read at stop.
static mut BATCH: [u64; 3] = [0; 3];
static mut RESULT: [u64; 3] = [0; 3];
static mut AUDIT: [u64; 2] = [0; 2];
static mut MED_READ: [u64; 2] = [0; 2];
static mut MED_SUM: u64 = 0;
static mut TIMES: [u64; 9] = [0; 9];
static INPUT: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

fn now() -> u64 {
    let t;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)t,options(nostack)) };
    t
}
fn stamp(i: usize) {
    unsafe { ptr::write_volatile((&raw mut TIMES).cast::<u64>().add(i), now()) }
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

#[inline(never)]
#[unsafe(no_mangle)]
fn mutex_batch_compute() -> [u64; 2] {
    let mut n = 0usize;
    let mut sum = 0u64;
    IN_WORK.store(1, SeqCst);
    for chunk in 0..256 {
        for _ in 0..256 {
            sum += unsafe { ptr::read_volatile(INPUT.as_ptr().add(n & 15)) } as u64;
            n += 1;
        }
        unsafe {
            ptr::write_volatile(
                (&raw mut LEDGER).cast::<[u64; 2]>().add(chunk),
                [n as u64, sum],
            )
        };
        CHUNKS.store((chunk + 1) as u64, SeqCst);
        if CEILING && chunk + 1 == 128 {
            checked(YIELD, 0)
        }
    }
    IN_WORK.store(0, SeqCst);
    [n as u64, sum]
}
extern "C" fn low() -> ! {
    checked(LOCK, X);
    if CEILING {
        checked(LOCK, Y)
    }
    stamp(0);
    let [count, sum] = mutex_batch_compute();
    unsafe {
        let b = (&raw mut BATCH).cast::<u64>();
        ptr::write_volatile(b.add(1), count);
        ptr::write_volatile(b.add(2), sum);
        ptr::write_volatile(b, 1);
    }
    stamp(1);
    checked(UNLOCK, if CEILING { Y } else { X });
    if CEILING {
        let version = unsafe { ptr::read_volatile((&raw const BATCH).cast::<u64>()) };
        unsafe { ptr::write_volatile(&raw mut AUDIT, [version, CHUNKS.load(SeqCst)]) };
        stamp(2);
        checked(UNLOCK, X)
    }
    exit()
}
extern "C" fn high() -> ! {
    stamp(3);
    checked(LOCK, if CEILING { Y } else { X });
    stamp(4);
    unsafe { ptr::write_volatile(&raw mut RESULT, ptr::read_volatile(&raw const BATCH)) };
    stamp(5);
    checked(UNLOCK, if CEILING { Y } else { X });
    exit()
}
extern "C" fn medium() -> ! {
    if CEILING {
        checked(LOCK, X);
        unsafe { ptr::write_volatile(&raw mut MED_READ, ptr::read_volatile(&raw const AUDIT)) };
        stamp(6);
        checked(UNLOCK, X)
    }
    let mut sum = 0u64;
    for i in 1u64..=256 {
        sum += unsafe { ptr::read_volatile(&i) }
    }
    unsafe { ptr::write_volatile(&raw mut MED_SUM, sum) };
    stamp(7);
    exit()
}
extern "C" fn idle() -> ! {
    loop {
        unsafe { asm!("wfi", options(nostack)) }
    }
}

struct Runtime {
    locks: LockSet<4, 2>,
    ready: ReadyQueue<4>,
    frames: [*mut Frame; 4],
    current: usize,
    done: u8,
    armed: bool,
    released: bool,
    blocked: bool,
    handoff: bool,
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
        let locks = &self.locks;
        self.current = self
            .ready
            .pop_highest(|i| locks.effective(i).unwrap_or(0))
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
            priorities |= u64::from(self.locks.effective(t).unwrap_or(0)) << (8 * t)
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
            ];
            USED += 1
        }
    }
}
extern "C" fn request(f: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1",out(reg)esr,options(nostack)) };
    if esr >> 26 != 0x15 || esr & 0xffff != 0 {
        stop(10)
    }
    let r = rt();
    let id = r.save(f);
    let (op, arg) = unsafe { ((*f).gpr[8], (*f).gpr[0] as usize) };
    if !CEILING && matches!(op, LOCK | UNLOCK) && arg != X {
        stop(6)
    }
    let mut result = 0;
    let mut arm = false;
    match op {
        LOCK => match r.locks.acquire(id, arg).unwrap_or_else(|| stop(6)) {
            Acquire::Held => {
                unsafe { (*f).gpr[0] = 0 };
                r.enqueue(id, true);
                arm = id == LOW && arg == (if CEILING { Y } else { X }) && !r.armed;
            }
            Acquire::Blocked => {
                result = NA;
                if id == HIGH {
                    r.blocked = true
                }
            }
        },
        UNLOCK => {
            if let Some(t) = r.locks.release(id, arg).unwrap_or_else(|| stop(7)) {
                let waiting = r.frames[t];
                if !owned_frame(t, waiting) {
                    stop(4)
                }
                unsafe { (*waiting).gpr[0] = 0 };
                r.enqueue(t, false);
                if t == HIGH && id == LOW {
                    stamp(8);
                    r.handoff = true
                }
            }
            unsafe { (*f).gpr[0] = 0 };
            r.enqueue(id, true)
        }
        YIELD => {
            if !CEILING || id != LOW || CHUNKS.load(SeqCst) != 128 {
                stop(8)
            }
            unsafe { (*f).gpr[0] = 0 };
            r.enqueue(id, false)
        }
        EXIT => {
            if id == IDLE || r.done & (1 << id) != 0 || (0..2).any(|l| r.locks.owner(l) == Some(id))
            {
                stop(9)
            }
            r.done |= 1 << id
        }
        _ => stop(10),
    }
    let next = r.choose();
    r.record(op, arg, id, f, next, result);
    if r.done == 7 {
        if !r.released || !r.blocked || !r.handoff {
            stop(15)
        }
        let batch = unsafe { ptr::read_volatile(&raw const BATCH) };
        let result = unsafe { ptr::read_volatile(&raw const RESULT) };
        let audit = unsafe { ptr::read_volatile(&raw const MED_READ) };
        if CHUNKS.load(SeqCst) != 256
            || batch != result
            || batch[0] != 1
            || (CEILING && audit != [batch[0], CHUNKS.load(SeqCst)])
            || (0..2).any(|lock| r.locks.owner(lock).is_some())
            || (0..4)
                .any(|id| r.locks.waiting(id).is_some() || r.locks.effective(id) != Some(PRI[id]))
        {
            stop(18)
        }
        stop(0)
    }
    if arm {
        if r.current != LOW || r.freq % 10000 != 0 || r.freq == 0 {
            stop(13)
        }
        let start = now();
        let target = start
            .checked_add(r.freq / 10000)
            .unwrap_or_else(|| stop(13));
        let (readback, ctl): (u64, u64);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            "mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0",in(reg)target,in(reg)1u64,
            out(reg)readback,out(reg)ctl,options(nostack));
            TIMER[0] = r.freq;
            TIMER[1] = start;
            TIMER[2] = target;
            TIMER[3] = readback;
            TIMER[4] = ctl;
        }
        r.armed = true;
        if readback != target || ctl & 3 != 1 {
            stop(13)
        }
    }
    next
}
extern "C" fn interrupt(f: *mut Frame) -> *mut Frame {
    let r = rt();
    let id = r.save(f);
    let iar = gic_read(GICC + 0xc);
    let ctl: u64;
    let cval: u64;
    unsafe {
        asm!("mrs {},cntp_ctl_el0","mrs {},cntp_cval_el0","msr cntp_ctl_el0,xzr","isb",
        out(reg)ctl,out(reg)cval,options(nostack));
        TIMER[5] = iar as u64;
        TIMER[6] = ctl;
        TIMER[7] = cval;
    }
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", "isb", options(nostack)) }
    if !r.armed
        || r.released
        || iar & 1023 != TIMER_IRQ
        || ctl & 7 != 5
        || cval != unsafe { TIMER[2] }
    {
        stop(14)
    }
    r.released = true;
    // An interrupted equal-priority owner keeps its place until explicit yield.
    r.enqueue(id, true);
    r.enqueue(HIGH, false);
    r.enqueue(MED, false);
    let next = r.choose();
    r.record(IRQ, 0, id, f, next, 0);
    if id != LOW
        || IN_WORK.load(SeqCst) != 1
        || unsafe { (*f).reserved } < cval
        || r.locks.owner(if CEILING { Y } else { X }) != Some(LOW)
        || (CEILING && CHUNKS.load(SeqCst) >= 128)
    {
        stop(16)
    }
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
    uart_write(b"MUTEX RAW 1\n");
    unsafe {
        let used = USED.min(ROWS);
        let returns = RETURNS[0].min(64) as usize;
        line(
            b"H",
            &[
                CEILING as u64,
                reason,
                used as u64,
                returns as u64,
                CHUNKS.load(SeqCst),
            ],
        );
        line(b"T", &*(&raw const TIMER));
        for id in 0..4 {
            line(
                b"S",
                &[
                    id as u64,
                    PRI[id] as u64,
                    (top(id) - STACK_BYTES) as u64,
                    top(id) as u64,
                    size_of::<Frame>() as u64,
                ],
            )
        }
        for i in 0..used {
            line(b"E", &*(&raw const JOURNAL[i]))
        }
        for i in 0..returns {
            let start = 1 + i * 3;
            let v = &*(&raw const RETURNS);
            line(b"R", &v[start..start + 3])
        }
        for i in 0..CHUNKS.load(SeqCst).min(256) as usize {
            line(b"C", &*(&raw const LEDGER[i]))
        }
        line(b"B", &*(&raw const BATCH));
        line(b"HIGH", &*(&raw const RESULT));
        line(b"A", &*(&raw const AUDIT));
        line(b"M", &[MED_READ[0], MED_READ[1], MED_SUM]);
        line(b"K", &*(&raw const TIMES));
    }
    uart_write(b"MUTEX END\n");
    park()
}
pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)freq,options(nostack)) }
    let mut r = Runtime {
        locks: locks(CEILING).unwrap_or_else(|| stop(1)),
        ready: ReadyQueue::new(),
        frames: [ptr::null_mut(); 4],
        current: IDLE,
        done: 0,
        armed: false,
        released: false,
        blocked: false,
        handoff: false,
        freq,
    };
    let entries: [extern "C" fn() -> !; 4] = [low, medium, high, idle];
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
    r.enqueue(IDLE, false);
    r.enqueue(LOW, false);
    let f = r.choose();
    unsafe {
        ptr::write(&raw mut ACTIVE, Some(r));
        sched_start_frame(f)
    }
}
