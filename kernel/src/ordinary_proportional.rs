//! Ordinary proportional scheduling on one virt CPU.
use super::{context::*, park, proportional_policy::Proportional, uart_write};
use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{
    AtomicPtr, AtomicU64, AtomicUsize,
    Ordering::{Acquire, Relaxed, Release},
};

const STRIDE: bool = cfg!(any(
    jnox_prop_case = "stride",
    jnox_prop_case = "stride-delay"
));
const DELAY: bool = cfg!(jnox_prop_case = "stride-delay");
const MISS: bool = cfg!(jnox_prop_case = "lottery-miss");
const SEED: u64 = if cfg!(jnox_prop_case = "lottery2") {
    2
} else {
    1
};
const MODE: u64 = if DELAY {
    4
} else if MISS {
    3
} else if STRIDE {
    2
} else {
    SEED - 1
};
const WEIGHTS: [u8; 3] = [1, 2, 4];
const IDLE: usize = 3;
const LIMIT: usize = 512;

global_asm!(".set JNOX_AP,1",include_str!("context.S"),frame_size=const size_of::<Frame>(),
    elr=const offset_of!(Frame,elr),fpcr=const offset_of!(Frame,fpcr),
    tpidr_el1=const offset_of!(Frame,tpidr_el1),tpidr_el0=const offset_of!(Frame,tpidr_el0),
    simd=const offset_of!(Frame,simd),irq=sym irq,svc=sym svc,fail=sym invalid);

#[repr(C)]
struct Returns {
    n: u64,
    rows: [[u64; 3]; 64],
}
#[unsafe(no_mangle)]
static mut jnox_ap_returns: Returns = Returns {
    n: 0,
    rows: [[0; 3]; 64],
};
static mut STACKS: [Stack; 4] = [const { Stack([0; STACK_BYTES]) }; 4];
static mut RUN: Option<Run> = None;
static mut LOG: [[u64; 7]; LIMIT] = [[0; 7]; LIMIT];
static mut USED: usize = 0;
static SLOT: AtomicU64 = AtomicU64::new(u64::MAX);
static YIELD_AT: AtomicU64 = AtomicU64::new(u64::MAX);
static DELAY_RELEASE_AT: AtomicU64 = AtomicU64::new(u64::MAX);
static mut FIRST_FAILURE: [u64; 5] = [0; 5];

fn counter() -> u64 {
    let t;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)t,options(nostack,preserves_flags)) };
    t
}
fn row(k: u64, data: [u64; 6]) {
    unsafe {
        if USED == LIMIT {
            uart_write(b"PROP OVERFLOW\n");
            park()
        }
        (&raw mut LOG)
            .cast::<[u64; 7]>()
            .add(USED)
            .write([k, data[0], data[1], data[2], data[3], data[4], data[5]]);
        USED += 1;
    }
}
struct Uart;
impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        uart_write(s.as_bytes());
        Ok(())
    }
}
fn dump() -> ! {
    unsafe {
        asm!(
            "msr daifset,#2",
            "msr cntp_ctl_el0,xzr",
            "isb",
            options(nostack)
        )
    };
    let mut u = Uart;
    let _ = writeln!(u, "PROP RAW 1");
    for n in 0..unsafe { USED } {
        let [k, a, b, c, d, e, f] = unsafe { (&raw const LOG).cast::<[u64; 7]>().add(n).read() };
        let _ = writeln!(u, "{k} {a} {b} {c} {d} {e} {f}");
    }
    let [line, mode, slot, actual, expected] = unsafe { FIRST_FAILURE };
    let _ = writeln!(u, "PROP END {line} {mode} {slot} {actual} {expected}");
    park()
}
fn fail(line: u32, actual: u64, expected: u64) -> ! {
    unsafe {
        asm!("msr daifset,#2", options(nostack));
        if FIRST_FAILURE[0] == 0 {
            FIRST_FAILURE = [line.into(), MODE, SLOT.load(Acquire), actual, expected]
        }
    }
    dump()
}
macro_rules! same {
    ($a:expr,$b:expr) => {{
        let (a, b) = ($a as u64, $b as u64);
        if a != b {
            fail(line!(), a, b)
        }
    }};
}
macro_rules! need {
    ($ok:expr) => {
        if !$ok {
            fail(line!(), 0, 1)
        }
    };
}
extern "C" fn invalid() -> ! {
    fail(line!(), 0, 1)
}
fn top(id: usize) -> usize {
    (&raw mut STACKS).addr() + (id + 1) * STACK_BYTES
}
fn within(id: usize, addr: usize, bytes: usize) -> bool {
    addr >= top(id) - STACK_BYTES && addr.checked_add(bytes).is_some_and(|end| end <= top(id))
}

// Only this worker writes its two cells on its own stack. It publishes a
// complete tuple before switching the active index, including across IRQs.
struct Local {
    cells: [[AtomicU64; 3]; 2],
    active: AtomicUsize,
}
static PTR: [AtomicPtr<Local>; 3] = [const { AtomicPtr::new(ptr::null_mut()) }; 3];
#[inline(always)]
fn work<const ID: usize>() -> ! {
    let mut deferring_irq = DELAY && ID == 0;
    if deferring_irq {
        // The first grant keeps its programmed compare while A finishes its work.
        unsafe { asm!("msr daifset,#2", "isb", options(nostack, preserves_flags)) };
    }
    let local = Local {
        cells: core::array::from_fn(|_| {
            [
                AtomicU64::new(0),
                AtomicU64::new(ID as u64 + 1),
                AtomicU64::new(0),
            ]
        }),
        active: AtomicUsize::new(0),
    };
    PTR[ID].store((&raw const local).cast_mut(), Release);
    let (mut x, mut count, mut bank) = (ID as u64 + 1, 0u64, 0usize);
    loop {
        for _ in 0..16 {
            x = x.wrapping_mul(3).wrapping_add(1)
        }
        count = count.wrapping_add(16);
        bank ^= 1;
        local.cells[bank][0].store(count, Relaxed);
        local.cells[bank][1].store(x, Relaxed);
        let stamp = counter();
        local.cells[bank][2].store(stamp, Relaxed);
        local.active.store(bank, Release);
        if deferring_irq && stamp >= DELAY_RELEASE_AT.load(Acquire) {
            deferring_irq = false;
            unsafe { asm!("msr daifclr,#2", "isb", options(nostack, preserves_flags)) };
        }
        if ID == 0 && STRIDE && !DELAY && counter() >= YIELD_AT.load(Acquire) {
            unsafe { asm!("svc #1", options(nostack)) }
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn prop_rust_a() -> ! {
    work::<0>()
}
#[unsafe(no_mangle)]
extern "C" fn prop_rust_b() -> ! {
    work::<1>()
}
#[unsafe(no_mangle)]
extern "C" fn prop_rust_c() -> ! {
    work::<2>()
}
extern "C" fn idle() -> ! {
    loop {
        unsafe { asm!("wfi", options(nostack)) }
    }
}

fn sample(id: usize) -> [u64; 4] {
    let p = PTR[id].load(Acquire);
    if p.is_null() {
        return [0, id as u64 + 1, 0, 0];
    }
    need!(within(id, p.addr(), size_of::<Local>()));
    let l = unsafe { &*p };
    let c = &l.cells[l.active.load(Acquire)];
    [
        c[0].load(Relaxed),
        c[1].load(Relaxed),
        c[2].load(Relaxed),
        p.addr() as u64,
    ]
}

fn advance(mut x: u64, mut steps: u64) -> u64 {
    let (mut a, mut b) = (3u64, 1u64);
    while steps != 0 {
        if steps & 1 != 0 {
            x = x.wrapping_mul(a).wrapping_add(b)
        }
        b = b.wrapping_mul(a).wrapping_add(b);
        a = a.wrapping_mul(a);
        steps >>= 1;
    }
    x
}

struct Run {
    policy: Proportional<3>,
    tick: u64,
    slot: usize,
    current: usize,
    start: u64,
    target: u64,
    frames: [*mut Frame; 4],
    selected: [u64; 2],
    saved: [Option<(u64, u64)>; 4],
    samples: [[u64; 4]; 3],
    counts: [u64; 3],
    service: [u64; 3],
}
fn state() -> &'static mut Run {
    unsafe { (*(&raw mut RUN)).as_mut().unwrap_or_else(|| invalid()) }
}
impl Run {
    fn passes(&self) -> [u64; 3] {
        core::array::from_fn(|i| self.policy.pass(i).unwrap_or(0))
    }
    fn arm(&mut self, ticks: u64) {
        self.start = counter();
        self.target = self
            .start
            .checked_add(self.tick.checked_mul(ticks).unwrap_or_else(|| invalid()))
            .unwrap_or_else(|| invalid());
        let c: u64;
        unsafe {
            asm!("msr cntp_cval_el0,{d}","msr cntp_ctl_el0,{on}","isb","mrs {c},cntp_cval_el0",
            d=in(reg) self.target,on=in(reg) 1u64,c=out(reg)c,options(nostack))
        };
        same!(c, self.target);
    }
    fn take(&mut self, id: usize) -> *mut Frame {
        let f = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        need!(within(id, f.addr(), size_of::<Frame>()));
        let saved = unsafe { &*f };
        let hash = saved
            .gpr
            .iter()
            .fold(saved.spsr, |h, r| h.rotate_left(7) ^ r);
        if let Some((pc, prior)) = self.saved[id] {
            same!(saved.elr, pc);
            same!(hash, prior)
        }
        same!(saved.tpidr_el1, id + 1);
        self.selected = [f.addr() as u64, saved.elr];
        row(
            14,
            [
                self.slot as u64,
                id as u64,
                0,
                f.addr() as u64,
                saved.elr,
                hash,
            ],
        );
        self.current = id;
        row(
            7,
            [
                self.slot as u64,
                id as u64,
                self.start,
                self.target,
                f.addr() as u64,
                unsafe { (*f).elr },
            ],
        );
        f
    }
    fn dispatch(&mut self) -> *mut Frame {
        SLOT.store(self.slot as u64, Release);
        let n = self.slot as u64;
        if STRIDE && !DELAY && self.slot == 8 {
            let [a, b, c] = self.passes();
            row(11, [n, a, b, c, 3, 7]);
            need!(self.policy.set_ready_mask(7));
        }
        let before = self.passes();
        let id = self.policy.dispatch().unwrap_or_else(|| invalid());
        self.counts[id] += 1;
        if let Some(draw) = self.policy.draw_record() {
            need!(self.policy.last_draw() == Some((draw.ticket, draw.total)));
            need!(self.policy.winner_for_draw(draw.ticket) == Some(id));
            let low: u64 = (0..id)
                .filter(|&i| self.policy.ready_mask() & (1 << i) != 0)
                .map(|i| u64::from(WEIGHTS[i]))
                .sum();
            row(
                3,
                [
                    n,
                    draw.before,
                    draw.after,
                    draw.words,
                    draw.raw,
                    draw.ticket.into(),
                ],
            );
            row(
                4,
                [
                    n,
                    self.policy.ready_mask().into(),
                    draw.total.into(),
                    low,
                    low + u64::from(WEIGHTS[id]),
                    id as u64,
                ],
            );
        } else {
            let [a, b, c] = before;
            row(
                5,
                [
                    n,
                    self.policy.ready_mask().into(),
                    a,
                    b,
                    c,
                    self.policy
                        .last_normalization()
                        .unwrap_or_else(|| invalid()),
                ],
            );
            let [a, b, c] = self.passes();
            row(6, [n, id as u64, a, b, c, 0]);
        }
        self.arm(if STRIDE || (MISS && n == 0) { 2 } else { 1 });
        if DELAY {
            DELAY_RELEASE_AT.store(
                self.target
                    .checked_add(self.tick)
                    .unwrap_or_else(|| invalid()),
                Release,
            );
        }
        YIELD_AT.store(self.start + self.tick, Release);
        self.take(id)
    }
    fn close(&mut self, f: *mut Frame, irq: bool) -> *mut Frame {
        let id = self.current;
        need!(within(id, f.addr(), size_of::<Frame>()) && self.frames[id].is_null());
        let saved = unsafe { &*f };
        let pc = saved.elr;
        let hash = saved
            .gpr
            .iter()
            .fold(saved.spsr, |h, r| h.rotate_left(7) ^ r);
        self.saved[id] = Some((pc, hash));
        same!(saved.tpidr_el1, id + 1);
        row(
            14,
            [self.slot as u64, id as u64, 1, f.addr() as u64, pc, hash],
        );
        let stop = unsafe { (*f).reserved };
        let returns = unsafe { (&raw const jnox_ap_returns.n).read_volatile() };
        same!(returns, if id == IDLE { 1 } else { self.slot + 2 });
        let [rf, rpc, ret] = unsafe {
            (&raw const jnox_ap_returns.rows)
                .cast::<[u64; 3]>()
                .add(returns as usize - 1)
                .read_volatile()
        };
        row(8, [self.slot as u64, id as u64, rf, rpc, ret, stop]);
        same!(rf, self.selected[0]);
        same!(rpc, self.selected[1]);
        need!(self.start <= ret && ret <= stop);
        self.frames[id] = f;
        if id == IDLE {
            need!(irq && stop >= self.target);
            need!(self.policy.set_ready_mask(if DELAY {
                1
            } else if STRIDE {
                3
            } else {
                7
            }));
            return self.dispatch();
        }
        let n = self.slot as u64;
        let elapsed = stop - self.start;
        let whole = elapsed / self.tick;
        let service = u32::try_from(whole).unwrap_or_else(|_| invalid());
        need!(self.policy.stop(id, service));
        self.service[id] = self.service[id]
            .checked_add(whole)
            .unwrap_or_else(|| invalid());
        row(9, [n, id as u64, irq as u64, pc, stop, whole]);
        let [a, b, c] = self.passes();
        row(10, [n, a, b, c, elapsed % self.tick, f.addr() as u64]);
        for j in 0..3 {
            let next = sample(j);
            let [count, x, t, p] = next;
            row(12, [n, j as u64, count, x, t, p]);
            let old = self.samples[j];
            if j == id {
                need!(count > old[0] && t >= ret && t <= stop);
                same!(advance(old[1], count - old[0]), x);
            } else {
                need!(next == old)
            }
            self.samples[j] = next;
        }
        let wanted_irq = !STRIDE || DELAY || id != 0;
        same!(irq, wanted_irq);
        if irq {
            need!(stop >= self.target)
        }
        if DELAY {
            need!(whole >= 3)
        } else {
            same!(whole, if STRIDE && id != 0 { 2 } else { 1 })
        }
        if STRIDE && !DELAY {
            let expected = if self.slot < 8 {
                self.slot % 2
            } else {
                [0, 1, 2, 2][self.slot % 4]
            };
            same!(id, expected);
            if self.slot == 7 {
                same!(self.service[0], 4);
                same!(self.service[1], 8);
                same!(self.service[2], 0);
            }
        }
        self.slot += 1;
        if self.slot
            == if DELAY {
                1
            } else if STRIDE {
                16
            } else {
                32
            }
        {
            if !STRIDE {
                let expected = if SEED == 1 { [10, 8, 14] } else { [5, 11, 16] };
                for i in 0..3 {
                    same!(self.counts[i], expected[i]);
                }
            } else if !DELAY {
                same!(self.service[0], 6);
                same!(self.service[1], 12);
                same!(self.service[2], 8);
            }
            dump()
        }
        self.dispatch()
    }
}

extern "C" fn irq(f: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {},cntp_ctl_el0",out(reg)ctl,options(nostack)) };
    same!(iar & 1023, TIMER_IRQ);
    same!(ctl & 7, 5);
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", "dsb sy", options(nostack)) };
    gic_write(GICC + 0x10, iar);
    row(13, [SLOT.load(Acquire), iar.into(), ctl, 0, 0, 0]);
    state().close(f, true)
}
extern "C" fn svc(f: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1","msr cntp_ctl_el0,xzr","isb",out(reg)esr,options(nostack)) };
    same!(esr >> 26, 0x15);
    same!(esr & 65535, 1);
    state().close(f, false)
}
pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)freq,options(nostack)) };
    need!(freq > 0 && freq % 100 == 0);
    let mut policy = if STRIDE {
        Proportional::stride(WEIGHTS)
    } else {
        Proportional::lottery(WEIGHTS, SEED)
    }
    .unwrap_or_else(|| invalid());
    let initial = policy.lottery_state();
    need!(policy.dispatch().is_none() && initial == policy.lottery_state());
    let entries = [
        prop_rust_a as extern "C" fn() -> !,
        prop_rust_b,
        prop_rust_c,
        idle,
    ];
    let frames = core::array::from_fn(|i| unsafe {
        let f = (top(i) - size_of::<Frame>()) as *mut Frame;
        (*f).elr = entries[i] as usize as u64;
        (*f).spsr = 0x345;
        (*f).tpidr_el1 = i as u64 + 1;
        row(
            2,
            [
                i as u64,
                (top(i) - STACK_BYTES) as u64,
                top(i) as u64,
                (*f).elr,
                (*f).tpidr_el1,
                0,
            ],
        );
        f
    });
    let tick = freq / if STRIDE { 25 } else { 100 };
    row(
        1,
        [MODE, if STRIDE { 0 } else { SEED }, freq, tick, 0x040201, 0],
    );
    unsafe {
        RUN = Some(Run {
            policy,
            tick,
            slot: 0,
            current: IDLE,
            start: 0,
            target: 0,
            frames,
            selected: [0; 2],
            saved: [None; 4],
            samples: [[0, 1, 0, 0], [0, 2, 0, 0], [0, 3, 0, 0]],
            counts: [0; 3],
            service: [0; 3],
        })
    };
    let s = state();
    s.arm(1);
    let f = s.take(IDLE);
    unsafe { sched_start_frame(f) }
}
