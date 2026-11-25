//! Partitioned native-v1 scheduling on the two identified Pi 4 cores.
use super::{
    COUNT, NATIVE_PRIORITIES, STACK_BOTTOM,
    STACK_TOP, bsp, clock_gettime, counter, decimal, head, monotonic_target, native_el0_mmu, park,
    uart_write, unsigned,
};
#[cfg(not(jnox_native_energy_placement))]
use super::{NATIVE_SMP_CPU_MASKS, NATIVE_SMP_RELEASE_CELLS};
#[cfg(jnox_native_energy_placement)]
const NATIVE_SMP_RELEASE_CELLS: [u64; COUNT] = [0; COUNT];
#[cfg(jnox_native_energy_placement)]
#[path = "native_energy_placement.rs"]
mod energy;
use bsp::Frame;
use core::{
    arch::asm,
    ptr,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::*},
};

const IDLE: usize = COUNT;
const ALL: u32 = (1 << COUNT) - 1;
const CAP: usize = 256;
const WORDS: usize = 37;
#[cfg(not(jnox_native_energy_placement))]
const _: () = assert!(COUNT >= 2 && COUNT <= 3);
#[cfg(jnox_native_energy_placement)]
const _: () = assert!(COUNT == 1);

fn mask(slot: usize) -> u8 {
    #[cfg(jnox_native_energy_placement)]
    { let _ = slot; energy::owner_mask() }
    #[cfg(not(jnox_native_energy_placement))]
    { NATIVE_SMP_CPU_MASKS[slot] }
}
const NONE: u64 = u64::MAX;

static ONLINE: AtomicBool = AtomicBool::new(false);
static PING: AtomicBool = AtomicBool::new(false);
static EPOCH: AtomicU64 = AtomicU64::new(0);
static HZ: AtomicU64 = AtomicU64::new(0);
static CAL1: AtomicU64 = AtomicU64::new(0);
static CAL0: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static RELEASED: AtomicU32 = AtomicU32::new(0);
// A nonempty batch has exactly one outstanding SGI1. More releases can
// coalesce before the receiver swaps this word at actual acknowledgment.
static MAIL: AtomicU32 = AtomicU32::new(0);
static EXITED: AtomicU32 = AtomicU32::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static PARKED: AtomicBool = AtomicBool::new(false);
static FAULT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum State {
    Absent,
    Ready(u64),
    Running(u64),
    Sleep(u64),
    Exit(i32),
}
struct Local {
    jobs: [State; COUNT],
    frames: [Frame; COUNT],
    charge: [u64; COUNT],
    owner: usize,
    arrivals: u32,
    serial: u64,
    grid: u64,
    compare: u64,
    used: usize,
    dropped: u64,
}
impl Local {
    const NEW: Self = Self {
        jobs: [State::Absent; COUNT],
        frames: [Frame::ZERO; COUNT],
        charge: [0; COUNT],
        owner: IDLE,
        arrivals: 0,
        serial: 0,
        grid: 0,
        compare: 0,
        used: 0,
        dropped: 0,
    };
    fn ready(&mut self, i: usize) {
        self.serial = add(self.serial, 1);
        self.jobs[i] = State::Ready(self.serial);
    }
    fn pick(&self) -> usize {
        (0..COUNT)
            .filter_map(|i| match self.jobs[i] {
                State::Ready(n) | State::Running(n) => {
                    Some((core::cmp::Reverse(NATIVE_PRIORITIES[i]), n, i))
                }
                _ => None,
            })
            .min()
            .map_or(IDLE, |(_, _, i)| i)
    }
}
static mut LOCAL: [Local; 2] = [Local::NEW, Local::NEW];
// Each row has two complete ABI frames in privileged RAM (outgoing,
// prepared). In particular this retains all Q registers and both TLS words.
#[unsafe(no_mangle)]
static mut native_smp_frames: [[[Frame; 2]; CAP]; 2] = [[[Frame::ZERO; 2]; CAP]; 2];
#[unsafe(no_mangle)]
static mut native_smp_events: [[[u64; WORDS]; CAP]; 2] = [[[0; WORDS]; CAP]; 2];

fn event_wait() {
    unsafe {
        asm!("wfe", options(nostack, preserves_flags));
    }
}
fn notify() {
    unsafe {
        asm!("dsb ish", "sev", options(nostack, preserves_flags));
    }
}
fn fatal(code: u64) -> ! {
    if !FAULT.swap(true, AcqRel) {
        uart_write(b"NATIVE SMP FAULT ");
        unsigned(code);
        uart_write(b"\n");
    }
    notify();
    park()
}
fn add(a: u64, b: u64) -> u64 {
    a.checked_add(b).unwrap_or_else(|| fatal(1))
}
fn mul(a: u64, b: u64) -> u64 {
    a.checked_mul(b).unwrap_or_else(|| fatal(1))
}
fn identity() -> (usize, u64) {
    let mpidr: u64;
    let freq: u64;
    unsafe {
        asm!("mrs {},mpidr_el1","mrs {},cntfrq_el0",out(reg)mpidr,out(reg)freq,options(nostack,preserves_flags));
    }
    let affinity = mpidr & 0xff00ffffff;
    if affinity > 1 {
        fatal(2);
    }
    (affinity as usize, freq)
}
fn ttbr() -> u64 {
    let value;
    unsafe {
        asm!("mrs {},ttbr0_el1",out(reg)value,options(nostack,preserves_flags));
    }
    value
}
fn receive_entry(cpu: usize) {
    if identity().0 != cpu || FAULT.load(Acquire) {
        fatal(3);
    }
}
fn epoch() -> u64 {
    EPOCH.load(Acquire)
}
fn q() -> u64 {
    HZ.load(Relaxed) / 50
}
fn due(cell: u64) -> u64 {
    add(epoch(), mul(cell, q()))
}
fn print(tag: &[u8], row: &[u64]) {
    uart_write(b"NATIVE SMP ");
    uart_write(tag);
    for &v in row {
        uart_write(b" ");
        unsigned(v);
    }
    uart_write(b"\n");
}
pub(super) fn start(hz: u64, entry: [u64; COUNT], sp: [u64; COUNT]) -> ! {
    #[cfg(not(jnox_native_energy_placement))]
    if !NATIVE_SMP_CPU_MASKS.contains(&1) || !NATIVE_SMP_CPU_MASKS.contains(&2) { fatal(4); }
    if identity() != (0, hz) || hz < 50 {
        fatal(4);
    }
    HZ.store(hz, Relaxed);
    for i in 0..COUNT {
        #[cfg(not(jnox_native_energy_placement))]
        if !matches!(mask(i), 1 | 2) || NATIVE_PRIORITIES[i] == 0 || NATIVE_SMP_RELEASE_CELLS[i] > 255
        {
            fatal(4);
        }
        let mut f = Frame::ZERO;
        f.pc = entry[i];
        f.sp = sp[i];
        f.pstate = 0x340;
        // CPU1 has not been released from its firmware spin slot yet.
        #[cfg(not(jnox_native_energy_placement))]
        unsafe { LOCAL[(mask(i) - 1) as usize].frames[i] = f; }
        #[cfg(jnox_native_energy_placement)]
        energy::initialize(f);
    }
    bsp::init_distributor();
    bsp::init_local(0);
    bsp::release_secondary();
    while !ONLINE.load(Acquire) {
        if FAULT.load(Acquire) {
            park();
        }
        event_wait();
    }
    CAL0[0].store(counter(), Relaxed);
    PING.store(true, Release);
    notify();
    while CAL1.load(Acquire) == 0 {
        event_wait();
    }
    CAL0[1].store(counter(), Relaxed);
    let e = add(counter(), q());
    add(e, mul(256, q()));
    EPOCH.store(e, Release);
    notify();
    launch(0)
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_entry(cpu: usize) -> ! {
    if cpu != 1 || identity().0 != 1 {
        fatal(5);
    }
    unsafe {
        native_el0_mmu::enable_secondary(0).unwrap_or_else(|_| fatal(6));
    }
    if identity().1 != HZ.load(Acquire) {
        fatal(5);
    }
    bsp::init_local(1);
    ONLINE.store(true, Release);
    notify();
    while !PING.load(Acquire) {
        event_wait();
    }
    CAL1.store(counter(), Release);
    notify();
    while epoch() == 0 {
        event_wait();
    }
    launch(1)
}
fn launch(cpu: usize) -> ! {
    let e = epoch();
    unsafe {
        LOCAL[cpu].compare = e;
    }
    bsp::arm(e);
    unsafe { bsp::enter(cpu, ptr::null()) }
}

fn publish(now: u64) -> (u32, u64, u64, u64) {
    let old = RELEASED.load(Relaxed);
    let mut new = 0;
    let mut remote = 0;
    #[cfg(jnox_native_energy_placement)]
    if old == 0 && now >= epoch() { energy::decide(now); }
    for i in 0..COUNT {
        if old & (1 << i) == 0 && due(NATIVE_SMP_RELEASE_CELLS[i]) <= now {
            new |= 1 << i;
            if mask(i) == 2 {
                remote |= 1 << i;
            }
        }
    }
    if new == 0 {
        return (0, 0, 0, 0);
    }
    RELEASED.store(old | new, Release);
    let published = counter();
    let mut send = 0;
    let mut sent = 0;
    if remote != 0 && MAIL.fetch_or(remote, AcqRel) == 0 {
        unsafe {
            asm!("dsb ishst", options(nostack, preserves_flags));
        }
        send = counter();
        bsp::send_sgi(1, 1);
        sent = counter();
    }
    (new, published, send, sent)
}
fn syscall(l: &mut Local, id: usize, f: &mut Frame, entry: u64) -> u64 {
    let op = f.x[8];
    let result = match op {
        1 => {
            if f.x[0] as i32 as i64 as u64 != f.x[0] {
                fatal(10);
            }
            #[cfg(jnox_native_energy_placement)]
            energy::exit(entry);
            l.jobs[id] = State::Exit(f.x[0] as i32);
            EXITED.fetch_or(1 << id, Release);
            0
        }
        6 => clock_gettime(f.x[0], f.x[1] as usize, entry),
        7 => {
            if f.x[0] != 1 || f.x[1] != 1 {
                -22
            } else {
                match monotonic_target(f.x[2] as usize) {
                    Err(e) => e,
                    Ok(t) if t.saturating_sub(entry) > i64::MAX as u64 => -75,
                    Ok(t) => {
                        if t > entry {
                            l.jobs[id] = State::Sleep(t);
                        }
                        0
                    }
                }
            }
        }
        8 => {
            l.ready(id);
            0
        }
        9..=19 => -95,
        _ => -38,
    };
    f.x[0] = result as u64;
    result as u64
}
fn record(cpu: usize, l: &mut Local, row: [u64; WORDS], src: Frame, dst: Frame) {
    let i = l.used;
    if i == CAP {
        l.dropped = l.dropped.saturating_add(1);
        return;
    }
    unsafe {
        native_smp_frames[cpu][i] = [src, dst];
        native_smp_events[cpu][i] = row;
    }
    l.used += 1;
}
fn select(cpu: usize, fp: *mut Frame, entry: u64, irq: Option<u32>, esr: u64) -> *const Frame {
    receive_entry(cpu);
    // Only this core accesses its element between boot publication and final
    // park. Exceptions enter masked. Never retain this borrow across ERET.
    let l = unsafe { &mut *(&raw mut LOCAL[cpu]) };
    let src = l.owner;
    if fp.is_null() != (src == IDLE) {
        fatal(7);
    }
    let (handler_sp, elr, spsr, prior_cval, prior_ctl): (u64, u64, u64, u64, u64);
    unsafe {
        asm!("mov {0}, sp","mrs {1}, elr_el1","mrs {2}, spsr_el1","mrs {3}, cntp_cval_el0","mrs {4}, cntp_ctl_el0",out(reg) handler_sp,out(reg) elr,out(reg) spsr,out(reg) prior_cval,out(reg) prior_ctl, options(nostack, preserves_flags));
    }
    let old = l.compare;
    let src_root = ttbr();
    let returned = bsp::return_stamp(cpu);
    let mut delta = 0;
    let mut arg = 0;
    let mut result = 0;
    let id = irq.map(|iar| iar & 1023);
    let op = irq.map_or_else(|| unsafe { (*fp).x[8] }, u64::from);
    let mut saved = Frame::ZERO;
    if src != IDLE {
        if native_el0_mmu::active() != Some(src) || mask(src) != (1 << cpu) {
            fatal(8);
        }
        saved = unsafe { *fp };
        if saved.pstate & 31 != 0
            || !native_el0_mmu::user_range(saved.pc as usize, 4, false)
            || !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&saved.sp)
            || saved.sp % 16 != 0
        {
            fatal(9);
        }
        if returned == 0 {
            fatal(9);
        }
        #[cfg(jnox_native_energy_placement)]
        energy::observed(returned, entry);
        delta = entry.checked_sub(returned).unwrap_or_else(|| fatal(9));
        l.charge[src] = add(l.charge[src], delta);
        let mut continuation = saved;
        if irq.is_none() {
            if esr >> 26 != 0x15 || esr & 0xffff != 0x4a01 {
                fatal(10);
            }
            arg = saved.x[0];
            result = syscall(l, src, &mut continuation, entry);
        }
        l.frames[src] = continuation;
    } else if irq.is_none() {
        fatal(10);
    }
    let now = counter();
    let (new, pub_stamp, sgir, sent) = if cpu == 0 { publish(now) } else { (0, 0, 0, 0) };
    let mail = if id == Some(1) {
        if cpu != 1 || irq.unwrap() >> 10 & 7 != 0 {
            fatal(11);
        }
        let value = MAIL.swap(0, AcqRel);
        if value == 0 {
            fatal(11);
        }
        value
    } else {
        0
    };
    let published = RELEASED.load(Acquire);
    let mut acquired = 0;
    let mut woke = 0;
    for i in 0..COUNT {
        if mask(i) != (1 << cpu) {
            continue;
        }
        if published & (1 << i) != 0 && l.arrivals & (1 << i) == 0 {
            #[cfg(jnox_native_energy_placement)]
            { l.frames[i] = energy::initial_frame(); }
            l.arrivals |= 1 << i;
            acquired |= 1 << i;
            l.ready(i);
        }
        if let State::Sleep(w) = l.jobs[i] {
            if w <= now {
                woke |= 1 << i;
                l.ready(i);
            }
        }
    }
    let mut skipped = 0;
    if now >= due(l.grid) {
        let next = (now - epoch()) / q() + 1;
        skipped = next - l.grid - 1;
        l.grid = next;
    }
    let target = l.pick();
    if target != src && src < COUNT {
        if let State::Running(ticket) = l.jobs[src] {
            l.jobs[src] = State::Ready(ticket);
        }
    }
    if target < COUNT {
        if let State::Ready(ticket) = l.jobs[target] {
            l.jobs[target] = State::Running(ticket);
        }
        if native_el0_mmu::active() != Some(target) {
            unsafe {
                native_el0_mmu::switch(target).unwrap_or_else(|_| fatal(12));
            }
        }
    }
    l.owner = target;
    let mut next = due(l.grid);
    for task in l.jobs {
        if let State::Sleep(w) = task {
            next = next.min(w);
        }
    }
    l.compare = next;
    bsp::arm(next);
    let (programmed_cval, programmed_ctl): (u64, u64);
    unsafe {
        asm!("isb","mrs {0}, cntp_cval_el0","mrs {1}, cntp_ctl_el0",out(reg) programmed_cval,out(reg) programmed_ctl,options(nostack,preserves_flags));
    }
    let prepared = if target == IDLE {
        Frame::ZERO
    } else {
        l.frames[target]
    };
    let mut ready = 0;
    for (i, s) in l.jobs.iter().enumerate() {
        if matches!(s, State::Ready(_) | State::Running(_)) {
            ready |= 1 << i;
        }
    }
    let row = [
        cpu as u64,
        l.used as u64,
        if irq.is_some() { 1 } else { 2 },
        op,
        entry,
        now,
        src as u64,
        target as u64,
        returned,
        delta,
        old,
        next,
        l.grid,
        skipped,
        new as u64,
        pub_stamp,
        sgir,
        mail as u64,
        acquired as u64,
        woke as u64,
        ready,
        src_root,
        ttbr(),
        arg,
        result,
        esr,
        bsp::idle_stamp(cpu),
        EXITED.load(Acquire) as u64,
        if src < COUNT { l.charge[src] } else { 0 },
        handler_sp,
        elr,
        spsr,
        prior_cval,
        prior_ctl,
        programmed_cval,
        programmed_ctl,
        sent,
    ];
    record(cpu, l, row, saved, prepared);
    if let Some(iar) = irq {
        if iar & 1023 < 1020 {
            bsp::eoi(iar);
        }
    }
    if id == Some(2) {
        if cpu != 1 || irq.unwrap() >> 10 & 7 != 0 || !STOP.load(Acquire) || target != IDLE {
            fatal(13);
        }
        bsp::disable_timer();
        PARKED.store(true, Release);
        notify();
        park();
    }
    let complete = EXITED.load(Acquire) == ALL;
    #[cfg(jnox_native_energy_placement)]
    let complete = complete || (RELEASED.load(Acquire) == ALL && mask(0) == 0);
    if cpu == 0 && complete && MAIL.load(Acquire) == 0 {
        if target != IDLE || RELEASED.load(Acquire) != ALL {
            fatal(13);
        }
        bsp::disable_timer();
        STOP.store(true, Release);
        unsafe {
            asm!("dsb ishst", options(nostack, preserves_flags));
        }
        bsp::send_sgi(1, 2);
        while !PARKED.load(Acquire) {
            if FAULT.load(Acquire) {
                park();
            }
            event_wait();
        }
        finish()
    }
    if target == IDLE {
        ptr::null()
    } else {
        &raw const l.frames[target]
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_irq(
    cpu: usize,
    frame: *mut Frame,
    entry: u64,
    iar: u32,
) -> *const Frame {
    select(cpu, frame, entry, Some(iar), 0)
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_sync(
    cpu: usize,
    frame: *mut Frame,
    entry: u64,
    esr: u64,
) -> *const Frame {
    if frame.is_null() {
        fatal(10);
    }
    select(cpu, frame, entry, None, esr)
}
fn finish() -> ! {
    print(
        b"CONFIG",
        &[1, HZ.load(Relaxed), q(), epoch(), COUNT as u64, IDLE as u64],
    );
    print(
        b"CAL",
        &[
            CAL0[0].load(Relaxed),
            CAL1.load(Acquire),
            CAL0[1].load(Relaxed),
        ],
    );
    for cpu in 0..2 {
        // CPU1 has released its final acknowledgment and cannot touch the
        // buffers again. CPU0 will never leave this terminal handler.
        let l = unsafe { &*(&raw const LOCAL[cpu]) };
        print(b"STACK", &bsp::stack_state(cpu));
        print(
            b"JOURNAL",
            &[
                cpu as u64,
                unsafe { (&raw const native_smp_events[cpu]).addr() as u64 },
                l.used as u64,
                WORDS as u64,
                l.dropped,
                unsafe { (&raw const native_smp_frames[cpu]).addr() as u64 },
                core::mem::size_of::<Frame>() as u64,
            ],
        );
        for i in 0..COUNT {
            if mask(i) != (1 << cpu) {
                continue;
            }
            if let State::Exit(s) = l.jobs[i] {
                print(
                    b"TASK",
                    &[
                        i as u64,
                        cpu as u64,
                        NATIVE_PRIORITIES[i] as u64,
                        NATIVE_SMP_RELEASE_CELLS[i],
                        l.charge[i],
                        native_el0_mmu::space_id(i).unwrap_or_else(|| fatal(14)),
                    ],
                );
                uart_write(b"NATIVE EL0 EXIT PID ");
                unsigned(i as u64);
                uart_write(b" ");
                decimal(i64::from(s));
                uart_write(b"\n");
            } else {
                fatal(14);
            }
        }
    }
    head(10, 1);
    head(1, 3);
    #[cfg(jnox_native_energy_placement)]
    energy::emit();
    print(b"END", &[1, u64::from(EXITED.load(Acquire)), 1, 1]);
    park()
}
