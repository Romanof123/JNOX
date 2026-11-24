//! CHG-070: one mailbox transfer at a time, between pinned ordinary Rust tasks.
use core::sync::atomic::{
    AtomicPtr, AtomicU64, AtomicUsize,
    Ordering::{AcqRel, Acquire, Relaxed, Release},
};
use core::{
    arch::asm,
    fmt::{self, Write},
    mem::size_of,
    ptr,
};

use crate::{
    context::{Frame, STACK_BYTES, sched_start_frame},
    pi4_smp_bsp as bsp,
};

const PRODUCER: usize = 0;
const BG: usize = 1;
const RX: usize = 2;
const CAP: usize = 32;
const CANARY: u64 = 0x534d_5031_4755_4152;
const UNSET: usize = usize::MAX;

#[repr(C, align(16))]
struct TaskStack {
    low: [u64; 2],
    bytes: [u8; STACK_BYTES],
    high: [u64; 2],
}

#[unsafe(no_mangle)]
static mut pi4_smp_task_stacks: [TaskStack; 3] = [const {
    TaskStack {
        low: [CANARY; 2],
        bytes: [0; STACK_BYTES],
        high: [CANARY; 2],
    }
}; 3];

fn stack(role: usize) -> *mut TaskStack {
    unsafe { (&raw mut pi4_smp_task_stacks).cast::<TaskStack>().add(role) }
}

fn bounds(role: usize) -> (usize, usize) {
    let low = unsafe { (&raw mut (*stack(role)).bytes).addr() };
    (low, low + STACK_BYTES)
}

fn guards(role: usize) -> bool {
    unsafe {
        ptr::read_volatile(&raw const (*stack(role)).low) == [CANARY; 2]
            && ptr::read_volatile(&raw const (*stack(role)).high) == [CANARY; 2]
    }
}

fn fresh(role: usize, entry: extern "C" fn() -> !) -> *mut Frame {
    let f = (bounds(role).1 - size_of::<Frame>()) as *mut Frame;
    unsafe {
        f.write_bytes(0, 1);
        (*f).elr = entry as usize as u64;
        (*f).spsr = 0x345;
    }
    f
}

static BG_FRAME: AtomicPtr<Frame> = AtomicPtr::new(ptr::null_mut());
static RX_FRAME: AtomicPtr<Frame> = AtomicPtr::new(ptr::null_mut());
static OWNER: AtomicUsize = AtomicUsize::new(UNSET); // CPU1's logical frame lease
static STATE: AtomicUsize = AtomicUsize::new(0); // blocked=4g, ready=4g+1, active=4g+2
static GENERATION: AtomicUsize = AtomicUsize::new(0);
static PAYLOAD: AtomicU64 = AtomicU64::new(0);
static RESULT: AtomicU64 = AtomicU64::new(0);
static ACK: AtomicUsize = AtomicUsize::new(0);
static PROGRESS: AtomicU64 = AtomicU64::new(0);
static RETURNED: AtomicUsize = AtomicUsize::new(UNSET);
static MILESTONE: AtomicUsize = AtomicUsize::new(0);
static STOPPED: AtomicUsize = AtomicUsize::new(0);
static ABORT: AtomicUsize = AtomicUsize::new(0);
static FAILURE: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];

struct Row {
    tick: AtomicU64,
    kind: AtomicUsize,
    generation: AtomicUsize,
    args: [AtomicU64; 5],
    committed: AtomicUsize,
}
struct Journal {
    used: AtomicUsize,
    lost: AtomicUsize,
    rows: [Row; CAP],
}
static LOG: [Journal; 2] = [const {
    Journal {
        used: AtomicUsize::new(0),
        lost: AtomicUsize::new(0),
        rows: [const {
            Row {
                tick: AtomicU64::new(0),
                kind: AtomicUsize::new(0),
                generation: AtomicUsize::new(0),
                args: [const { AtomicU64::new(0) }; 5],
                committed: AtomicUsize::new(0),
            }
        }; CAP],
    }
}; 2];

fn event(cpu: usize, kind: usize, g: usize, args: [u64; 5]) {
    let j = &LOG[cpu];
    let seq = j.used.fetch_add(1, Relaxed);
    if seq >= CAP {
        j.lost.fetch_add(1, Relaxed);
        stop(cpu, 1);
    }
    let r = &j.rows[seq];
    r.tick.store(bsp::ticks(), Relaxed);
    r.kind.store(kind, Relaxed);
    r.generation.store(g, Relaxed);
    for (word, arg) in r.args.iter().zip(args) {
        word.store(arg, Relaxed)
    }
    r.committed.store(seq + 1, Release);
}

fn park() -> ! {
    loop {
        unsafe { asm!("wfe", options(nomem, nostack)) }
    }
}
fn mask_irq() {
    unsafe { asm!("msr daifset, #2", "isb", options(nostack)) }
}
fn limit(cpu: usize) -> u64 {
    let hz = bsp::frequency();
    if hz == 0 {
        stop(cpu, 2)
    }
    hz.checked_mul(4)
        .and_then(|v| bsp::ticks().checked_add(v))
        .unwrap_or_else(|| stop(cpu, 2))
}
fn check_time(cpu: usize, deadline: u64) {
    if bsp::ticks() >= deadline || bsp::failure() != 0 || ABORT.load(Acquire) != 0 {
        stop(cpu, 3)
    }
}
fn wait(cpu: usize, done: impl Fn() -> bool) {
    let deadline = limit(cpu);
    while !done() {
        check_time(cpu, deadline);
        core::hint::spin_loop();
    }
}

fn stop(cpu: usize, code: usize) -> ! {
    mask_irq();
    FAILURE[cpu].store(code, Release);
    ABORT.store(1, Release);
    if cpu == 1 {
        STOPPED.store(1, Release);
        park()
    }
    // A halted/offline peer cannot be presumed quiescent. Only emit a local
    // failure line on that path; do not read or export its journal.
    let start = bsp::ticks();
    let bound = bsp::frequency().saturating_mul(2);
    while STOPPED.load(Acquire) == 0 && bsp::ticks().wrapping_sub(start) < bound {
        core::hint::spin_loop()
    }
    if STOPPED.load(Acquire) != 0 {
        export(false)
    }
    let _ = writeln!(
        Serial,
        "SW FAIL local={} bsp={} iar={} peer_quiet=0",
        code,
        bsp::failure(),
        bsp::failure_iar()
    );
    park()
}

fn frame_event(cpu: usize, kind: usize, g: usize, role: usize, frame: *mut Frame) {
    let (lo, hi) = bounds(role);
    if frame.addr() < lo
        || frame.addr() > hi - size_of::<Frame>()
        || !frame.is_aligned()
        || !guards(role)
    {
        stop(cpu, 4)
    }
    event(
        cpu,
        kind,
        g,
        [
            role as u64,
            frame.addr() as u64,
            unsafe { (*frame).elr },
            (frame.addr() + size_of::<Frame>()) as u64,
            PROGRESS.load(Acquire),
        ],
    );
}

// Inline the probe so the retained PC/SP belong to the linked Rust task.
#[inline(always)]
fn work(cpu: usize, g: usize, role: usize, value: u64) {
    let sp: u64;
    let pc: u64;
    unsafe {
        asm!("mov {}, sp", "adr {}, .", out(reg) sp, out(reg) pc, options(nostack, preserves_flags))
    }
    let (lo, hi) = bounds(role);
    let (mpidr, _) = bsp::identity();
    if mpidr & 0xff00ffffff != cpu as u64
        || !(lo < sp as usize && sp as usize <= hi)
        || sp % 16 != 0
        || !guards(role)
    {
        stop(cpu, 5)
    }
    event(cpu, 3, g, [role as u64, pc, sp, value, mpidr]);
}

pub(super) fn start(cpu: usize) -> ! {
    let (mpidr, el) = bsp::identity();
    event(
        cpu,
        1,
        0,
        [
            mpidr,
            el,
            bsp::frequency(),
            bsp::ticks(),
            size_of::<Frame>() as u64,
        ],
    );
    let (role, f) = if cpu == 0 {
        let f = fresh(PRODUCER, pi4_smp_producer);
        if !bsp::release_cpu1() {
            stop(0, 6)
        }
        (PRODUCER, f)
    } else {
        BG_FRAME.store(fresh(BG, pi4_smp_background), Release);
        OWNER.store(RX, Release);
        (RX, fresh(RX, pi4_smp_receiver))
    };
    frame_event(cpu, 2, 0, role, f);
    unsafe { sched_start_frame(f) }
}

#[unsafe(no_mangle)]
extern "C" fn pi4_smp_producer() -> ! {
    for g in 1..=2 {
        wait(0, || MILESTONE.load(Acquire) == g);
        let prior = PROGRESS.load(Acquire);
        let mut sum = 0u64;
        for i in 1..=16 {
            sum += core::hint::black_box((16 * (g - 1) + i) as u64)
        }
        work(0, g, PRODUCER, sum);
        wait(0, || PROGRESS.load(Acquire) > prior);
        if STATE.load(Acquire) != 4 * g || OWNER.load(Acquire) != BG || ACK.load(Acquire) != g - 1 {
            stop(0, 7)
        }
        PAYLOAD.store(sum, Relaxed);
        GENERATION.store(g, Release);
        event(0, 4, g, [sum, prior, PROGRESS.load(Acquire), 0, 0]);
        let before = bsp::ticks();
        let sgir = bsp::send_sgi1();
        let after = bsp::ticks();
        let target = bsp::gic_state(1)[1];
        event(0, 5, g, [before, after, sgir as u64, target, sum]);
        if target.count_ones() != 1 || sgir as u64 != ((target << 16) | (1 << 15) | 1) {
            stop(0, 8)
        }
        wait(0, || ACK.load(Acquire) == g);
        let result = RESULT.load(Relaxed);
        event(0, 6, g, [sum, result, 0, 0, 0]);
        if result != 2 * sum {
            stop(0, 9)
        }
    }
    wait(0, || STOPPED.load(Acquire) != 0);
    mask_irq();
    if MILESTONE.load(Acquire) != 3
        || ABORT.load(Acquire) != 0
        || bsp::failure() != 0
        || !(0..3).all(guards)
    {
        stop(0, 10)
    }
    export(true)
}

#[inline(always)]
fn block(g: usize) {
    unsafe { asm!("svc #1", inout("x0") g => _, options(nostack)) };
}

#[unsafe(no_mangle)]
extern "C" fn pi4_smp_receiver() -> ! {
    work(1, 0, RX, 0);
    for g in 1..=2 {
        block(g);
        if OWNER.load(Acquire) != RX
            || STATE
                .compare_exchange(4 * g + 1, 4 * g + 2, AcqRel, Acquire)
                .is_err()
            || GENERATION.load(Acquire) != g
            || ACK.load(Acquire) != g - 1
        {
            stop(1, 11)
        }
        let payload = PAYLOAD.load(Relaxed);
        if payload != [136, 392][g - 1] {
            stop(1, 12)
        }
        let answer = core::hint::black_box(payload) * 2;
        work(1, g, RX, answer);
        RESULT.store(answer, Relaxed);
        event(1, 7, g, [payload, answer, 0, 0, 0]);
        ACK.store(g, Release);
    }
    block(3); // exit: this frame must never be selected again
    stop(1, 13)
}

#[unsafe(no_mangle)]
extern "C" fn pi4_smp_background() -> ! {
    let deadline = limit(1);
    let mut count = 0u64;
    let mut last = UNSET;
    loop {
        let before = RETURNED.load(Acquire);
        for _ in 0..16 {
            count = core::hint::black_box(count.wrapping_add(1))
        }
        PROGRESS.store(count, Release);
        if before != last && before != UNSET && RETURNED.load(Acquire) == before {
            if before > 2 || OWNER.load(Acquire) != BG {
                stop(1, 14)
            }
            work(1, before, BG, count);
            last = before;
            MILESTONE.store(before + 1, Release);
            if before == 2 {
                unsafe { asm!("svc #2", options(noreturn)) }
            }
        }
        if count & 1023 == 0 {
            check_time(1, deadline)
        }
    }
}

// BSP has read IAR exactly once. Its wrapper EOIs the full value after this
// callback, then calls eoi_done before restoring the selected receiver frame.
pub(super) fn irq(cpu: usize, frame: *mut Frame, iar: u32) -> *mut Frame {
    let g = GENERATION.load(Acquire);
    if cpu != 1 || !(1..=2).contains(&g) {
        stop(cpu, 15)
    }
    let source = (iar >> 10) & 7;
    let sender_target = bsp::gic_state(0)[1];
    if iar & 1023 != 1
        || sender_target != 1u64 << source
        || MILESTONE.load(Acquire) != g
        || OWNER.load(Acquire) != BG
    {
        stop(1, 16)
    }
    frame_event(1, 8, g, BG, frame);
    if STATE
        .compare_exchange(4 * g, 4 * g + 1, AcqRel, Acquire)
        .is_err()
        || !BG_FRAME.swap(frame, AcqRel).is_null()
    {
        stop(1, 17)
    }
    event(
        1,
        9,
        g,
        [
            iar as u64,
            source as u64,
            PAYLOAD.load(Relaxed),
            (4 * g) as u64,
            (4 * g + 1) as u64,
        ],
    );
    let next = RX_FRAME.swap(ptr::null_mut(), AcqRel);
    frame_event(1, 10, g, RX, next);
    OWNER.store(RX, Release);
    next
}

pub(super) fn eoi_done(cpu: usize, iar: u32) {
    event(
        cpu,
        11,
        GENERATION.load(Acquire),
        [iar as u64, OWNER.load(Acquire) as u64, 0, 0, 0],
    );
}

pub(super) fn svc(cpu: usize, frame: *mut Frame, imm: u16) -> *mut Frame {
    if cpu != 1 {
        stop(cpu, 18)
    }
    if imm == 2 && OWNER.load(Acquire) == BG && MILESTONE.load(Acquire) == 3 {
        frame_event(1, 15, 2, BG, frame);
        mask_irq();
        OWNER.store(UNSET, Release);
        STOPPED.store(1, Release);
        park()
    }
    if imm != 1 || OWNER.load(Acquire) != RX {
        stop(1, 19)
    }
    frame_event(1, 12, ACK.load(Acquire), RX, frame);
    let g = unsafe { (*frame).gpr[0] as usize };
    if !(1..=3).contains(&g)
        || ACK.load(Acquire) != g - 1
        || STATE.load(Acquire) != (if g == 1 { 0 } else { 4 * (g - 1) + 2 })
    {
        stop(1, 20)
    }
    if g <= 2 && !RX_FRAME.swap(frame, AcqRel).is_null() {
        stop(1, 21)
    }
    STATE.store(4 * g, Release);
    event(1, 13, g, [imm as u64, (4 * g) as u64, 0, 0, 0]);
    let next = BG_FRAME.swap(ptr::null_mut(), AcqRel);
    frame_event(1, 14, g - 1, BG, next);
    OWNER.store(BG, Release);
    RETURNED.store(g - 1, Release);
    next
}

struct Serial;
impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::uart_write(s.as_bytes());
        Ok(())
    }
}

fn export(success: bool) -> ! {
    let mut uart = Serial;
    let _ = writeln!(
        uart,
        "SW HEADER v=1 cap={} frame={}",
        CAP,
        size_of::<Frame>()
    );
    for role in 0..3 {
        let (lo, hi) = bounds(role);
        let _ = writeln!(
            uart,
            "SW STACK {} {} {} {}",
            role,
            lo,
            hi,
            guards(role) as u8
        );
    }
    let mut valid = success;
    for cpu in 0..2 {
        let j = &LOG[cpu];
        let used = j.used.load(Acquire);
        let lost = j.lost.load(Acquire);
        valid &= lost == 0 && used <= CAP;
        let _ = write!(
            uart,
            "SW CORE {} {} {} {}",
            cpu,
            used,
            lost,
            FAILURE[cpu].load(Acquire)
        );
        for word in bsp::boot_state(cpu).into_iter().chain(bsp::gic_state(cpu)) {
            let _ = write!(uart, " {}", word);
        }
        let _ = writeln!(uart);
        for (i, r) in j.rows.iter().take(used).enumerate() {
            if r.committed.load(Acquire) != i + 1 {
                valid = false;
                break;
            }
            let _ = write!(
                uart,
                "SW ROW {} {} {} {} {}",
                cpu,
                i + 1,
                r.kind.load(Relaxed),
                r.generation.load(Relaxed),
                r.tick.load(Relaxed)
            );
            for word in &r.args {
                let _ = write!(uart, " {}", word.load(Relaxed));
            }
            let _ = writeln!(uart);
        }
    }
    let _ = writeln!(
        uart,
        "SW END {} {} {} {}",
        valid as u8,
        ACK.load(Acquire),
        RESULT.load(Acquire),
        PROGRESS.load(Acquire)
    );
    park()
}
