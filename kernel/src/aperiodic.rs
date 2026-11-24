//! Timer-backed request service with a synchronous producer on its own stack.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::aperiodic_policy::ServerKind;
use super::context::*;
use super::request_server::{Receipt, RequestServer, ServerSnapshot};
use super::{park, uart_write};

const WORKER: usize = 0;
const IDLE: usize = 1;
const PRODUCER: usize = 2;
const RELEASES: [(u64, u32, u32); 4] = [(1, 1, 1), (3, 2, 3), (4, 3, 1), (7, 4, 1)];
const NAMES: [&[u8]; 4] = [b"PS", b"DS", b"SS", b"SS1"];

unsafe extern "C" {
    fn aper_worker();
    fn aper_idle();
    fn aper_producer();
    static aper_worker_begin: u8;
    static aper_worker_end: u8;
    static aper_idle_begin: u8;
    static aper_idle_end: u8;
    static aper_reply_1: u8;
    static aper_reply_2: u8;
    static aper_reply_3: u8;
    static aper_reply_4: u8;
    static aper_park_1: u8;
    static aper_park_2: u8;
    static aper_park_3: u8;
    static aper_park_4: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("aperiodic.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym producer_call, fail = sym fail,
);

enum Server {
    Normal(RequestServer<4, 2>),
    Limited(RequestServer<4, 1>),
}

// Only the credit queue bound differs. All operations use the real FIFO.
macro_rules! server {
    ($s:expr, $name:ident($($arg:expr),*)) => {
        match $s {
            Server::Normal(s) => s.$name($($arg),*),
            Server::Limited(s) => s.$name($($arg),*),
        }
    };
}

struct Epoch {
    index: usize,
    server: Server,
    base: u64,
    period: u64,
    tick: u64,
    releases: usize,
    submitted: bool,
    current: usize,
    frames: [*mut Frame; 3],
    pcs: [u64; 3],
    counts: [u64; 3],
    receipt: Option<Receipt>,
}

static mut STACKS: [Stack; 12] = [const { Stack([0; STACK_BYTES]) }; 12];
static mut ACTIVE: Option<Epoch> = None;

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"APER FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let value;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack,preserves_flags)) };
    value
}

fn number(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&buf[i..]);
}

fn slot_trace(tick: u64, id: u32, state: ServerSnapshot<'_>) {
    uart_write(b"APER t=");
    number(tick);
    uart_write(b" id=");
    number(id.into());
    uart_write(b" budget=");
    number(state.remaining_quota.into());
    uart_write(b" credits=");
    if state.replenishments.is_empty() {
        uart_write(b"-")
    }
    for (i, credit) in state.replenishments.iter().enumerate() {
        if i != 0 {
            uart_write(b",")
        }
        number(credit.due);
        uart_write(b":");
        number(credit.amount.into());
    }
    uart_write(b"\n");
}

// Single CPU, entered only at masked boot or from the shared exception vectors.
fn with_epoch<T>(f: impl FnOnce(&mut Epoch) -> T) -> T {
    let masks: u64;
    unsafe { asm!("mrs {}, daif",out(reg)masks,options(nostack,preserves_flags)) };
    require(masks & 0x80 != 0);
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Epoch {
    fn new(index: usize, period: u64) -> Self {
        let kind = match index {
            0 => ServerKind::Polling,
            1 => ServerKind::Deferrable,
            2 | 3 => ServerKind::Sporadic,
            _ => fail(),
        };
        let key = index as u64 + 1;
        let server = if index == 3 {
            Server::Limited(RequestServer::new(kind, 0, 2, 4, key).unwrap_or_else(|| fail()))
        } else {
            Server::Normal(RequestServer::new(kind, 0, 2, 4, key).unwrap_or_else(|| fail()))
        };
        Self {
            index,
            server,
            base: 0,
            period,
            tick: 0,
            releases: 0,
            submitted: false,
            current: IDLE,
            frames: [ptr::null_mut(); 3],
            pcs: [0; 3],
            counts: [0; 3],
            receipt: None,
        }
    }

    fn top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.index * 3 + id + 1) * STACK_BYTES)
    }

    fn witness(&self, id: usize) -> u64 {
        0xa501 + (self.index * 3 + id) as u64
    }

    fn start(&mut self) -> *mut Frame {
        let entries: [unsafe extern "C" fn(); 3] = [aper_worker, aper_idle, aper_producer];
        for (id, entry) in entries.into_iter().enumerate() {
            // Each epoch has fresh zeroed stacks. Initial return pops the
            // whole frame, and each assembly entry then reserves two words.
            unsafe {
                let f = self.top(id).sub(size_of::<Frame>()).cast::<Frame>();
                (*f).elr = entry as usize as u64;
                (*f).spsr = 0x3c5;
                (*f).gpr[19] = self.witness(id);
                self.frames[id] = f;
            }
        }
        uart_write(b"APER BEGIN ");
        uart_write(NAMES[self.index]);
        uart_write(b"\n");
        self.base = counter();
        self.grant()
    }

    fn deadline(&self, tick: u64) -> u64 {
        self.period
            .checked_mul(tick)
            .and_then(|d| self.base.checked_add(d))
            .unwrap_or_else(|| fail())
    }

    fn take(&mut self, id: usize) -> *mut Frame {
        let f = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!f.is_null());
        if self.pcs[id] != 0 {
            let saved = unsafe { &*f };
            require(saved.elr == self.pcs[id] && saved.gpr[20] == self.counts[id]);
            require(saved.gpr[19] == self.witness(id) && saved.tpidr_el1 == self.witness(id));
        }
        self.current = id;
        f
    }

    fn save(&mut self, f: *mut Frame) {
        let id = self.current;
        require(self.frames[id].is_null());
        require(f.addr() == self.top(id).addr() - 16 - size_of::<Frame>());
        let s = unsafe { &*f };
        require(s.gpr[19] == self.witness(id) && s.tpidr_el1 == self.witness(id));
        require(
            unsafe { ptr::read_volatile(self.top(id).sub(16).cast::<u64>()) } == self.witness(id),
        );
        if id != PRODUCER {
            let (begin, end) = if id == WORKER {
                (&raw const aper_worker_begin, &raw const aper_worker_end)
            } else {
                (&raw const aper_idle_begin, &raw const aper_idle_end)
            };
            require((begin.addr()..end.addr()).contains(&(s.elr as usize)));
            require(s.spsr & 0x8f == 5 && s.gpr[20] > self.counts[id]);
            let request = self.receipt.map_or(0, Receipt::id);
            require(s.gpr[0] == u64::from(request) && s.gpr[22] == u64::from(request));
        } else {
            require(s.spsr & 0x8f == 0x85)
        }
        self.frames[id] = f;
        self.pcs[id] = s.elr;
        self.counts[id] = s.gpr[20];
    }

    fn grant(&mut self) -> *mut Frame {
        require(self.receipt.is_none() && server!(&self.server, snapshot()).now == self.tick);
        self.receipt = server!(&mut self.server, grant(true)).unwrap_or_else(|_| fail());
        let id = if let Some(receipt) = self.receipt {
            require(receipt.tick() == self.tick && receipt.id() != 0);
            WORKER
        } else {
            IDLE
        };
        let request = self.receipt.map_or(0, Receipt::id);
        let state = server!(&self.server, snapshot());
        require(state.now == self.tick + 1 && state.in_flight == self.receipt);
        slot_trace(self.tick, request, state);
        let f = self.take(id);
        unsafe { (*f).gpr[0] = request.into() };
        let end = self.deadline(self.tick + 1);
        require(counter() < end);
        unsafe {
            asm!("msr cntp_cval_el0, {}","msr cntp_ctl_el0, {}","isb",in(reg) end,in(reg) 1u64,options(nostack,preserves_flags))
        };
        f
    }

    fn finish_slot(&mut self, f: *mut Frame) -> *mut Frame {
        require(self.current == if self.receipt.is_some() { WORKER } else { IDLE });
        self.save(f);
        let state = server!(&self.server, snapshot());
        require(state.in_flight == self.receipt && state.now == self.tick + 1);
        if let Some(r) = self.receipt.take() {
            require(r.tick() == self.tick);
            let before = server!(&self.server, remaining_work(r.id())).unwrap_or_else(|| fail());
            server!(&mut self.server, complete(r)).unwrap_or_else(|_| fail());
            require(server!(&self.server, remaining_work(r.id())).unwrap_or(0) == before - 1);
        }
        self.tick += 1;
        if self.tick == 10 {
            let state = server!(&self.server, snapshot());
            require(self.releases == 4 && !self.submitted && state.in_flight.is_none());
            require(
                state.charged == state.completed
                    && u64::from(state.queued_work) + state.completed == 6,
            );
            uart_write(b"APER END pending=");
            number(state.queued_work.into());
            uart_write(b" charged=");
            number(state.charged);
            uart_write(b" completed=");
            number(state.completed);
            uart_write(b"\n");
            if self.index == 3 {
                uart_write(b"APER STOP\nAPER PASS\n");
                park()
            }
            *self = Self::new(self.index + 1, self.period);
            return self.start();
        }
        if RELEASES
            .get(self.releases)
            .is_some_and(|r| r.0 == self.tick)
        {
            self.take(PRODUCER)
        } else {
            self.grant()
        }
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {}, cntfrq_el0",out(reg)freq,options(nostack,preserves_flags)) };
    let period = freq / 20;
    require(period > 0);
    unsafe {
        ACTIVE = Some(Epoch::new(0, period));
        sched_start_frame(with_epoch(Epoch::start))
    }
}

extern "C" fn producer_call(f: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1",out(reg)esr,options(nostack,preserves_flags)) };
    require(esr >> 26 == 0x15);
    with_epoch(|e| {
        require(e.current == PRODUCER && e.receipt.is_none());
        let &(tick, id, work) = RELEASES.get(e.releases).unwrap_or_else(|| fail());
        require(tick == e.tick);
        e.save(f);
        let s = unsafe { &mut *f };
        let index = e.releases;
        match esr & 0xffff {
            1 => {
                let replies = [
                    &raw const aper_reply_1,
                    &raw const aper_reply_2,
                    &raw const aper_reply_3,
                    &raw const aper_reply_4,
                ];
                require(!e.submitted && s.elr == replies[index].addr() as u64);
                require(s.gpr[0] == u64::from(id) && s.gpr[1] == u64::from(work));
                server!(&mut e.server, submit(s.gpr[0] as u32, s.gpr[1] as u32))
                    .unwrap_or_else(|_| fail());
                require(server!(&e.server, remaining_work(id)) == Some(work));
                e.submitted = true;
                s.gpr[0] = 0x100 + u64::from(id);
                e.take(PRODUCER)
            }
            2 => {
                let parks = [
                    &raw const aper_park_1,
                    &raw const aper_park_2,
                    &raw const aper_park_3,
                    &raw const aper_park_4,
                ];
                require(
                    e.submitted
                        && s.elr == parks[index].addr() as u64
                        && s.gpr[0] == 0x100 + u64::from(id),
                );
                e.submitted = false;
                e.releases += 1;
                e.grant()
            }
            _ => fail(),
        }
    })
}

extern "C" fn timer_irq(f: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0",out(reg)control,options(nostack,preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
    unsafe {
        asm!("msr cntp_ctl_el0, {}","isb","dsb sy",in(reg) 2u64,options(nostack,preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_epoch(|e| {
        let now = counter();
        require((e.deadline(e.tick + 1)..e.deadline(e.tick + 2)).contains(&now));
        e.finish_slot(f)
    })
}
