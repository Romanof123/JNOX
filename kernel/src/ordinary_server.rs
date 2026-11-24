//! Single CPU, sixteen absolute slots. Only masked exception handlers own the ledger.
use super::{
    aperiodic_policy::ServerKind,
    context::*,
    park,
    request_server::{Receipt, RequestServer},
    uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
};

global_asm!(".set JNOX_AP,1",include_str!("context.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym interrupt, svc = sym request, fail = sym fault);

const APP: &str = env!("JNOX_AP_APP");
const CREDITS: usize = if APP.as_bytes()[APP.len() - 1] == b'1' {
    1
} else {
    2
};
const A: bool = APP.as_bytes()[0] == b'a';
const IDLE: usize = 0;
const PRODUCER: usize = 1;
const SERVER: usize = 2;
const PERIODIC: usize = 3;
const COUNT: usize = 256;
const LIMIT: usize = 192;
static mut STACKS: [Stack; 4] = [const { Stack([0; STACK_BYTES]) }; 4];
static mut STATE: Option<Runtime> = None;
static mut EVENTS: [[u64; 10]; LIMIT] = [[0; 10]; LIMIT];
static mut USED: usize = 0;
// Task publishes each sample before advancing its count. The IRQ can stop a partial job.
static mut STEPS: [[[u64; 2]; COUNT]; 16] = [[[0; 2]; COUNT]; 16];
static mut LENGTHS: [usize; 16] = [0; 16];
#[unsafe(export_name = "jnox_ap_returns")]
static mut RETURNS: [u64; 193] = [0; 193];
static mut MASK: [u64; 2] = [0; 2];

fn now() -> u64 {
    let n;
    unsafe { asm!("isb","mrs {}, cntpct_el0",out(reg)n,options(nostack)) };
    n
}
fn word(n: u64) {
    let mut digits = [0; 16];
    for (i, c) in digits.iter_mut().enumerate() {
        *c = b"0123456789abcdef"[((n >> (60 - i * 4)) & 15) as usize];
    }
    uart_write(b" ");
    uart_write(&digits);
}
fn line(tag: &[u8], fields: &[u64]) {
    uart_write(tag);
    for &n in fields {
        word(n)
    }
    uart_write(b"\n");
}
fn event(kind: u64, slot: u64, tick: u64, data: [u64; 7]) {
    unsafe {
        if USED >= LIMIT {
            finish(99);
        }
        let row = &mut (*(&raw mut EVENTS))[USED];
        row[..3].copy_from_slice(&[kind, slot, tick]);
        row[3..].copy_from_slice(&data);
        USED += 1;
    }
}
fn finish(code: u64) -> ! {
    unsafe {
        asm!(
            "msr daifset,#2",
            "msr cntp_ctl_el0,xzr",
            "isb",
            options(nostack)
        );
    }
    for i in 0..unsafe { USED } {
        line(b"AP EVENT", &unsafe { EVENTS[i] });
    }
    for s in 0..16 {
        for i in 0..unsafe { ptr::read_volatile((&raw const LENGTHS).cast::<usize>().add(s)) } {
            let [t, x] = unsafe {
                ptr::read_volatile((&raw const STEPS).cast::<[u64; 2]>().add(s * COUNT + i))
            };
            line(b"AP STEP", &[s as u64, i as u64, t, x]);
        }
    }
    for i in 0..unsafe { RETURNS[0] as usize }.min(64) {
        line(b"AP RETURN", unsafe {
            &(&*(&raw const RETURNS))[1 + 3 * i..4 + 3 * i]
        });
    }
    line(b"AP MASK", &unsafe { MASK });
    line(b"AP END", &[code]);
    park()
}
extern "C" fn fault() -> ! {
    finish(90)
}
struct Runtime {
    ledger: RequestServer<4, CREDITS>,
    frequency: u64,
    epoch: u64,
    q: u64,
    next: u64,
    owner: usize,
    idle: *mut Frame,
    ticket: Option<Receipt>,
    sizes: [u32; 4],
    submits: [u64; 4],
    requested: u64,
    seed: u32,
    periodic: u64,
}
fn rt() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap() }
}
fn arrival(slot: u64) -> Option<(u32, u32)> {
    match (A, slot) {
        (true, 1) => Some((1, 1)),
        (true, 3) => Some((2, 2)),
        (true, 4) => Some((3, 1)),
        (false, 0) => Some((1, 4)),
        (false, 4) => Some((2, 2)),
        _ => None,
    }
}
fn make(owner: usize, entry: usize, args: [u64; 3]) -> *mut Frame {
    unsafe {
        let f = (&raw mut STACKS)
            .cast::<Stack>()
            .add(owner + 1)
            .cast::<Frame>()
            .sub(1);
        ptr::write_bytes(f, 0, 1);
        (*f).elr = entry as u64;
        (*f).spsr = 0x345;
        (&mut (*f).gpr)[..3].copy_from_slice(&args);
        f
    }
}
fn arm(target: u64) {
    unsafe {
        asm!("msr cntp_cval_el0,{}","msr cntp_ctl_el0,{}","isb",in(reg)target,in(reg)1u64,options(nostack))
    }
}
extern "C" fn idle(_epoch: u64, _q: u64) -> ! {
    loop {
        unsafe { asm!("wfi", options(nostack)) }
    }
}
extern "C" fn producer(id: u64, units: u64) -> ! {
    let submitted = now();
    unsafe { asm!("svc #1",in("x0")id,in("x1")units,in("x2")submitted,options(noreturn)) }
}
#[inline(never)]
extern "C" fn work(seed: u64, slot: usize, identity: u64, _unmask_at: u64) -> ! {
    let start = now();
    let mut x = seed as u32;
    for i in 0..COUNT {
        #[cfg(jnox_ap_late)]
        if i == COUNT / 2 && identity == 1 && _unmask_at != 0 {
            unsafe {
                asm!("msr daifset,#2", "isb");
                MASK[0] = now();
                while now() < _unmask_at {
                    core::hint::spin_loop();
                }
                MASK[1] = now();
                asm!("msr daifclr,#2", "isb");
            }
        }
        x = x.wrapping_mul(3).wrapping_add(1);
        unsafe {
            ptr::write_volatile(
                (&raw mut STEPS).cast::<[u64; 2]>().add(slot * COUNT + i),
                [now(), x as u64],
            );
            ptr::write_volatile((&raw mut LENGTHS).cast::<usize>().add(slot), i + 1);
        }
    }
    let done = now();
    unsafe {
        asm!("svc #2",in("x0")identity,in("x1")x as u64,in("x2")start,in("x3")done,options(noreturn))
    }
}
fn handoff(r: &mut Runtime, slot: u64, owner: usize, frame: *mut Frame) -> *mut Frame {
    let t = now();
    let target = r.epoch + slot * r.q;
    event(
        4,
        slot,
        t,
        [
            r.owner as u64,
            owner as u64,
            frame.addr() as u64,
            unsafe { (*frame).elr },
            target,
            0,
            0,
        ],
    );
    if owner != IDLE && now() - target > r.frequency / 200 {
        finish(4)
    }
    r.owner = owner;
    frame
}
fn decide(r: &mut Runtime, slot: u64) -> *mut Frame {
    let periodic = slot % 4 == 2;
    let ticket = r.ledger.grant(!periodic).unwrap();
    let ss = r.ledger.snapshot();
    event(
        3,
        slot,
        now(),
        [
            ss.remaining_quota as u64,
            ss.queued_work as u64,
            ss.charged,
            ss.completed,
            ticket.map_or(0, |v| v.id() as u64),
            ss.replenishments.len() as u64,
            periodic as u64,
        ],
    );
    for c in ss.replenishments {
        event(8, slot, now(), [c.due, c.amount as u64, 0, 0, 0, 0, 0]);
    }
    // Acceptance observer checks the actual grant after arbitration.
    let required: &[u64] = match APP {
        "a-ps" => &[4, 5, 8, 9],
        "a-ds" => &[1, 3, 4, 5],
        "a-ss2" => &[1, 3, 5, 7],
        "a-ss1" => &[1, 3, 7, 8],
        "b-ss1" => &[0, 1, 5, 7, 11, 12],
        _ => &[0, 1, 4, 5, 8, 9],
    };
    if ticket.is_some() != required.contains(&slot) {
        finish(9);
    }
    let (owner, identity, unit) = if periodic {
        (PERIODIC, slot / 4 + 1, 0)
    } else if let Some(t) = ticket {
        let id = t.id();
        let unit = r.sizes[id as usize] - r.ledger.remaining_work(id).unwrap();
        r.ticket = Some(t);
        (SERVER, id as u64, unit)
    } else {
        let idle = r.idle;
        let frame = handoff(r, slot, IDLE, idle);
        if now() - (r.epoch + slot * r.q) > r.frequency / 200 {
            finish(4)
        }
        return frame;
    };
    r.seed = identity as u32 + unit;
    let f = make(
        owner,
        work as *const () as usize,
        [r.seed as u64, slot, identity],
    );
    if owner == SERVER {
        unsafe {
            (*f).gpr[3] = r.epoch + (slot + 2) * r.q + r.q / 2;
        }
    }
    handoff(r, slot, owner, f)
}
extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let t = unsafe { (*frame).reserved };
    let iar = gic_read(GICC + 0x0c);
    if iar & 1023 == 1023 {
        return frame;
    }
    if iar & 1023 != TIMER_IRQ {
        fault()
    }
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", "dsb sy", options(nostack)) }
    gic_write(GICC + 0x10, iar);
    let r = rt();
    let slot = r.next;
    let target = r.epoch + slot * r.q;
    let elapsed = t.saturating_sub(target);
    let ss = r.ledger.snapshot();
    event(
        1,
        slot,
        t,
        [
            target,
            r.owner as u64,
            frame.addr() as u64,
            unsafe { (*frame).elr },
            elapsed / r.q,
            ss.completed,
            ss.queued_work as u64,
        ],
    );
    if elapsed > r.frequency / 1000 || t < target || r.owner != IDLE || r.ticket.is_some() {
        finish(1)
    }
    if r.requested != ss.completed + ss.queued_work as u64 {
        finish(2)
    }
    r.idle = frame;
    if slot == 16 {
        if ss.queued_work != 0 || r.requested != if A { 4 } else { 6 } || r.periodic != 4 {
            finish(3)
        }
        finish(0)
    }
    r.next += 1;
    arm(target + r.q);
    if let Some((id, n)) = arrival(slot) {
        let f = make(
            PRODUCER,
            producer as *const () as usize,
            [id as u64, n as u64, 0],
        );
        handoff(r, slot, PRODUCER, f)
    } else {
        decide(r, slot)
    }
}
extern "C" fn request(frame: *mut Frame) -> *mut Frame {
    let t = unsafe { (*frame).reserved };
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1",out(reg)esr) }
    let r = rt();
    let slot = r.next - 1;
    let target = r.epoch + slot * r.q;
    let regs = unsafe { &(*frame).gpr };
    event(
        5,
        slot,
        t,
        [
            r.owner as u64,
            frame.addr() as u64,
            unsafe { (*frame).elr },
            regs[0],
            regs[1],
            regs[2],
            regs[3],
        ],
    );
    if esr >> 26 != 0x15 {
        fault()
    }
    match (esr & 0xffff, r.owner) {
        (1, PRODUCER) => {
            let (id, n) = arrival(slot).unwrap();
            if (regs[0], regs[1]) != (id as u64, n as u64)
                || regs[2] < target
                || regs[2] > t
                || t - target > r.frequency / 200
            {
                finish(5)
            }
            r.ledger.submit_bounded(id, n).unwrap();
            r.sizes[id as usize] = n;
            r.submits[id as usize] = regs[2];
            r.requested += n as u64;
            event(
                2,
                slot,
                now(),
                [id as u64, n as u64, regs[2], r.requested, 0, 0, 0],
            );
            decide(r, slot)
        }
        (2, SERVER | PERIODIC) => {
            let done = regs[3];
            if regs[2] < target
                || done < regs[2]
                || done > t
                || t > target + r.q
                || unsafe { LENGTHS[slot as usize] } != COUNT
            {
                finish(6)
            }
            let mut expected = r.seed;
            for _ in 0..COUNT {
                expected = expected.wrapping_mul(3).wrapping_add(1)
            }
            if regs[1] != expected as u64 {
                finish(7)
            }
            let latency = if r.owner == SERVER {
                let ticket = r.ticket.unwrap();
                if ticket.tick() != slot || regs[0] != ticket.id() as u64 {
                    finish(8)
                }
                r.ledger.complete(ticket).unwrap();
                r.ticket = None;
                done - r.submits[ticket.id() as usize]
            } else {
                if regs[0] != slot / 4 + 1 {
                    finish(8)
                }
                r.periodic += 1;
                done - target
            };
            event(
                6,
                slot,
                now(),
                [
                    r.owner as u64,
                    regs[0],
                    regs[1],
                    regs[2],
                    done,
                    latency,
                    r.ledger.snapshot().completed,
                ],
            );
            let idle = r.idle;
            handoff(r, slot, IDLE, idle)
        }
        _ => fault(),
    }
}
pub(crate) fn run() -> ! {
    #[cfg(not(jnox_ap_admission))]
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)frequency) }
    #[cfg(jnox_ap_admission)]
    {
        let report = include_str!(concat!(env!("OUT_DIR"), "/ap-report.txt"));
        if let Err(reason) = super::aperiodic_admission::check(report, APP, frequency) {
            line(b"AP ADMISSION DENIED", &[reason as u64]);
            park()
        }
        uart_write(b"AP ADMISSION BUDGET ");
        uart_write(APP.as_bytes());
        uart_write(b" ");
        uart_write(option_env!("JNOX_AP_APPROVED_SHA256").unwrap().as_bytes());
        uart_write(b"\n");
        init_timer_irq();
    }
    if frequency == 0 || frequency % 1000 != 0 {
        fault()
    }
    let q = frequency / 20;
    let epoch = now().checked_add(q).unwrap();
    epoch.checked_add(17 * q).unwrap();
    let kind = match APP.as_bytes()[2] {
        b'p' => ServerKind::Polling,
        b'd' => ServerKind::Deferrable,
        _ => ServerKind::Sporadic,
    };
    let idle_frame = make(IDLE, idle as *const () as usize, [epoch, q, 0]);
    let base = (&raw const STACKS).addr() as u64;
    event(
        0,
        0,
        now(),
        [
            frequency,
            epoch,
            q,
            base,
            STACK_BYTES as u64,
            CREDITS as u64,
            idle_frame.addr() as u64,
        ],
    );
    unsafe {
        STATE = Some(Runtime {
            ledger: RequestServer::new(kind, 0, 2, 4, 1).unwrap(),
            frequency,
            epoch,
            q,
            next: 0,
            owner: IDLE,
            idle: idle_frame,
            ticket: None,
            sizes: [0; 4],
            submits: [0; 4],
            requested: 0,
            seed: 0,
            periodic: 0,
        });
        arm(epoch);
        sched_start_frame(idle_frame)
    }
}
