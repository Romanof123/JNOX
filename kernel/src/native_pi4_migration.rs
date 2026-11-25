//! Native global/clustered dispatch. Shared state is accessed only while the
//! calling CPU has masked IRQs and owns the scheduler ticket.
use super::{
    COUNT, NATIVE_PRIORITIES, NATIVE_SMP_CPU_MASKS, NATIVE_SMP_EFFECTIVE_MASKS,
    NATIVE_SMP_ONLINE_CPUS as CPUS, NATIVE_SMP_RELEASE_CELLS, STACK_BOTTOM, STACK_TOP, bsp,
    clock_gettime, counter, decimal, head, monotonic_target, native_el0_mmu, park, uart_write,
    unsigned,
};
use crate::{
    domain_policy::AffinityDomain,
    smp_policy::{Disposition, Lease, Ownership},
};
use bsp::Frame;
use core::{
    arch::asm,
    cell::UnsafeCell,
    ptr,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::*},
};

const IDLE: usize = COUNT;
const ONLINE: u8 = (1 << CPUS) - 1;
const ALL: u32 = (1 << COUNT) - 1;
const CAP: usize = 256;
const W: usize = 44;
const _: () = assert!(CPUS >= 2 && CPUS <= 3 && COUNT >= 2 && COUNT <= 3);
static HZ: AtomicU64 = AtomicU64::new(0);
static E: AtomicU64 = AtomicU64::new(0);
static UP: AtomicU32 = AtomicU32::new(0);
static PING: AtomicBool = AtomicBool::new(false);
static CAL: [AtomicU64; CPUS] = [const { AtomicU64::new(0) }; CPUS];
static BEFORE: AtomicU64 = AtomicU64::new(0);
static AFTER: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicBool = AtomicBool::new(false);
static PARKED: AtomicU32 = AtomicU32::new(0);
fn wait() {
    unsafe {
        asm!("wfe", options(nostack, preserves_flags));
    }
}
fn wake() {
    unsafe {
        asm!("dsb ish", "sev", options(nostack, preserves_flags));
    }
}
fn fatal(n: u64) -> ! {
    if !FAILED.swap(true, AcqRel) {
        print(b"FAULT", &[n]);
    }
    wake();
    park()
}
fn add(a: u64, b: u64) -> u64 {
    a.checked_add(b).unwrap_or_else(|| fatal(1))
}
fn q() -> u64 {
    HZ.load(Acquire) / 50
}
fn due(k: u64) -> u64 {
    add(
        E.load(Acquire),
        k.checked_mul(q()).unwrap_or_else(|| fatal(1)),
    )
}
fn root() -> u64 {
    let r;
    unsafe {
        asm!("mrs {},ttbr0_el1",out(reg)r,options(nostack,preserves_flags));
    }
    r
}
fn ident() -> (usize, u64) {
    let (m, f): (u64, u64);
    unsafe {
        asm!("mrs {},mpidr_el1","mrs {},cntfrq_el0",out(reg)m,out(reg)f,options(nostack,preserves_flags));
    }
    ((m & 0xff00ffffff) as usize, f)
}
fn print(tag: &[u8], v: &[u64]) {
    uart_write(b"NATIVE MIG ");
    uart_write(tag);
    for &x in v {
        uart_write(b" ");
        unsigned(x);
    }
    uart_write(b"\n");
}

#[derive(Clone, Copy)]
enum Job {
    Absent,
    Active,
    Sleep(u64),
    Exit(i32),
}
struct State {
    owners: Ownership<COUNT>,
    domain: AffinityDomain<COUNT>,
    leases: [Option<Lease>; CPUS],
    job: [Job; COUNT],
    age: [u64; COUNT],
    serial: u64,
    frames: [Frame; COUNT],
    stop_acks: u8,
    pending: u8,
    released: u32,
    exited: u32,
    stopping: bool,
    revision: u64,
    compare: [u64; CPUS],
    used: [usize; CPUS],
    dropped: [u64; CPUS],
}
impl State {
    fn ready(&mut self, p: usize) {
        self.serial = add(self.serial, 1);
        self.age[p] = self.serial;
        self.job[p] = Job::Active;
    }
    fn schedule(&self, old: [usize; CPUS]) -> [usize; CPUS] {
        let mut ranked = [0u8; COUNT];
        let mut n = 0;
        for p in 0..COUNT {
            if matches!(self.job[p], Job::Active) {
                ranked[n] = p as u8;
                n += 1;
            }
        }
        ranked[..n].sort_unstable_by_key(|&p| {
            (
                core::cmp::Reverse(NATIVE_PRIORITIES[p as usize]),
                self.age[p as usize],
                p,
            )
        });
        let map = self
            .domain
            .place(ONLINE, &ranked[..n])
            .unwrap_or_else(|| fatal(2));
        let mut best = core::array::from_fn(|c| map[c].map_or(IDLE, usize::from));
        let chosen = best
            .iter()
            .filter(|&&p| p != IDLE)
            .fold(0u8, |m, &p| m | (1 << p));
        let mut best_key = (usize::MAX, [IDLE; CPUS]);
        // At most 4^3 assignments. Preserve the matcher's selected set, then
        // retain eligible incumbents and use CPU order as the final tie break.
        for mut code in 0..(COUNT + 1).pow(CPUS as u32) {
            let mut a = [IDLE; CPUS];
            let mut seen = 0u8;
            let mut valid = true;
            for (c, v) in a.iter_mut().enumerate() {
                *v = code % (COUNT + 1);
                code /= COUNT + 1;
                if *v != IDLE {
                    let bit = 1 << *v;
                    if seen & bit != 0 || NATIVE_SMP_EFFECTIVE_MASKS[*v] & (1 << c) == 0 {
                        valid = false;
                        break;
                    }
                    seen |= bit;
                }
            }
            if valid && seen == chosen {
                let moved = (0..CPUS)
                    .filter(|&c| old[c] != IDLE && old[c] != a[c])
                    .count();
                if (moved, a) < best_key {
                    best_key = (moved, a);
                    best = a;
                }
            }
        }
        best
    }
}
struct Shared(UnsafeCell<Option<State>>);
// Safety: with_state is the sole accessor; ticket ordering transfers ownership.
unsafe impl Sync for Shared {}
static SHARED: Shared = Shared(UnsafeCell::new(None));
static TICKET: AtomicU64 = AtomicU64::new(0);
static SERVING: AtomicU64 = AtomicU64::new(0);
fn with_state<T>(f: impl FnOnce(&mut Option<State>) -> T) -> T {
    let daif: u64;
    unsafe {
        asm!("mrs {},daif",out(reg)daif,options(nostack,preserves_flags));
    }
    if daif & 128 == 0 {
        fatal(3);
    }
    let ticket = TICKET.fetch_add(1, Relaxed);
    while SERVING.load(Acquire) != ticket {
        if FAILED.load(Acquire) {
            park();
        }
        core::hint::spin_loop();
    }
    let result = f(unsafe { &mut *SHARED.0.get() });
    SERVING.store(ticket.wrapping_add(1), Release);
    result
}
#[unsafe(no_mangle)]
static mut native_mig_frames: [[[Frame; 2]; CAP]; CPUS] = [[[Frame::ZERO; 2]; CAP]; CPUS];
#[unsafe(no_mangle)]
static mut native_mig_events: [[[u64; W]; CAP]; CPUS] = [[[0; W]; CAP]; CPUS];
static mut INCOMING: [Frame; CPUS] = [Frame::ZERO; CPUS];

pub(super) fn start(hz: u64, entry: [u64; COUNT], sp: [u64; COUNT]) -> ! {
    if ident() != (0, hz) || hz < 50 {
        fatal(4);
    }
    HZ.store(hz, Release);
    let mut frames = [Frame::ZERO; COUNT];
    for p in 0..COUNT {
        let mask = NATIVE_SMP_EFFECTIVE_MASKS[p];
        if mask == 0
            || mask & !NATIVE_SMP_CPU_MASKS[p] != 0
            || NATIVE_SMP_CPU_MASKS[p] & !ONLINE != 0
            || NATIVE_PRIORITIES[p] == 0
            || NATIVE_SMP_RELEASE_CELLS[p] > 255
        {
            fatal(4);
        }
        frames[p].pc = entry[p];
        frames[p].sp = sp[p];
        frames[p].pstate = 0x340;
    }
    with_state(|s| {
        *s = Some(State {
            owners: Ownership::new(NATIVE_SMP_EFFECTIVE_MASKS).unwrap_or_else(|| fatal(4)),
            domain: AffinityDomain::new(NATIVE_SMP_EFFECTIVE_MASKS).unwrap_or_else(|| fatal(4)),
            leases: [None; CPUS],
            job: [Job::Absent; COUNT],
            age: [0; COUNT],
            serial: 0,
            frames,
            stop_acks: 0,
            pending: 0,
            released: 0,
            exited: 0,
            stopping: false,
            revision: 0,
            compare: [0; CPUS],
            used: [0; CPUS],
            dropped: [0; CPUS],
        })
    });
    bsp::init_distributor();
    bsp::init_local(0);
    bsp::release_secondary();
    while UP.load(Acquire) != u32::from(ONLINE & !1) {
        wait();
    }
    BEFORE.store(counter(), Relaxed);
    PING.store(true, Release);
    wake();
    while CAL[1..].iter().any(|s| s.load(Acquire) == 0) {
        wait();
    }
    AFTER.store(counter(), Relaxed);
    let epoch = add(counter(), q());
    E.store(epoch, Release);
    due(256);
    wake();
    launch(0)
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_entry(cpu: usize) -> ! {
    if cpu == 0 || cpu >= CPUS || ident().0 != cpu {
        fatal(5);
    }
    unsafe {
        native_el0_mmu::enable_secondary(0).unwrap_or_else(|_| fatal(5));
    }
    if ident().1 != HZ.load(Acquire) {
        fatal(5);
    }
    bsp::init_local(cpu);
    UP.fetch_or(1 << cpu, Release);
    wake();
    while !PING.load(Acquire) {
        wait();
    }
    CAL[cpu].store(counter(), Release);
    wake();
    while E.load(Acquire) == 0 {
        wait();
    }
    launch(cpu)
}
fn launch(cpu: usize) -> ! {
    let e = E.load(Acquire);
    with_state(|s| s.as_mut().unwrap().compare[cpu] = e);
    bsp::arm(e);
    unsafe { bsp::enter(cpu, ptr::null()) }
}

fn call(s: &mut State, p: usize, f: &mut Frame, at: u64) -> u64 {
    let arg = f.x[0];
    let result: i64 = match f.x[8] {
        1 => {
            if arg as i32 as i64 as u64 != arg {
                fatal(6);
            }
            s.job[p] = Job::Exit(arg as i32);
            s.exited |= 1 << p;
            0
        }
        6 => clock_gettime(arg, f.x[1] as usize, at),
        7 if arg != 1 || f.x[1] != 1 => -22,
        7 => match monotonic_target(f.x[2] as usize) {
            Err(e) => e,
            Ok(w) if w.saturating_sub(at) > i64::MAX as u64 => -75,
            Ok(w) => {
                if w > at {
                    s.job[p] = Job::Sleep(w);
                }
                0
            }
        },
        8 => {
            s.ready(p);
            0
        }
        9..=19 => -95,
        _ => -38,
    };
    f.x[0] = result as u64;
    result as u64
}
struct Transfer {
    index: usize,
    target: usize,
    send: u8,
    stop: u8,
    park: bool,
    end: bool,
    next: u64,
}
fn dispatch(cpu: usize, fp: *mut Frame, entry: u64, irq: Option<u32>, esr: u64) -> *const Frame {
    if cpu >= CPUS || ident().0 != cpu || FAILED.load(Acquire) {
        fatal(7);
    }
    let saved = if fp.is_null() {
        Frame::ZERO
    } else {
        unsafe { *fp }
    };
    let prior = root();
    let r = bsp::return_stamp(cpu);
    let id = irq.map(|i| i & 1023);
    let op = irq.map_or(saved.x[8], u64::from);
    let (actual, ctl, handler_sp): (u64, u64, u64);
    unsafe {
        asm!("mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0","mov {},sp",out(reg)actual,out(reg)ctl,out(reg)handler_sp,options(nostack,preserves_flags));
    }
    let t = with_state(|slot| {
        let s = slot.as_mut().unwrap();
        let old = core::array::from_fn(|c| s.leases[c].map_or(IDLE, |l| l.task as usize));
        let src = old[cpu];
        let mut resumed = saved;
        let mut delta = 0;
        let mut verdict = 0;
        let outgoing = s.leases[cpu].take();
        if fp.is_null() != (src == IDLE) {
            fatal(8);
        }
        if let Some(lease) = outgoing {
            if native_el0_mmu::active() != Some(src)
                || saved.pstate & 31 != 0
                || !native_el0_mmu::user_range(saved.pc as usize, 4, false)
                || !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&saved.sp)
                || saved.sp % 16 != 0
                || r == 0
            {
                fatal(8);
            }
            delta = entry.checked_sub(r).unwrap_or_else(|| fatal(8));
            if irq.is_none() {
                if esr >> 26 != 0x15 || esr & 65535 != 0x4a01 {
                    fatal(9);
                }
                verdict = call(s, src, &mut resumed, entry);
            }
            let disposition = if matches!(s.job[src], Job::Active) {
                Disposition::Ready
            } else {
                Disposition::Dormant
            };
            s.frames[src] = resumed;
            if !s.owners.retire(lease, delta, disposition) {
                fatal(10);
            }
        } else if irq.is_none() {
            fatal(9);
        }
        if matches!(id, Some(1 | 2)) && ((irq.unwrap() >> 10) & 7) as usize >= CPUS {
            fatal(11);
        }
        if id == Some(1) {
            s.pending &= !(1 << cpu);
        }
        let now = counter();
        let mut releases = 0u32;
        let mut wakes = 0u32;
        for p in 0..COUNT {
            match s.job[p] {
                Job::Absent if cpu == 0 && now >= due(NATIVE_SMP_RELEASE_CELLS[p]) => {
                    s.ready(p);
                    releases |= 1 << p;
                }
                Job::Sleep(w) if now >= w => {
                    s.ready(p);
                    wakes |= 1 << p;
                }
                _ => {}
            }
        }
        if releases | wakes != 0 && !s.owners.release(releases | wakes) {
            fatal(12);
        }
        s.released |= releases;
        let proposed = s.schedule(old);

        s.revision = add(s.revision, 1);
        let want = proposed[cpu];
        let mut target = IDLE;
        let mut seq = 0;
        if want != IDLE && !s.leases.iter().flatten().any(|l| l.task as usize == want) {
            let lease = s
                .owners
                .claim(cpu as u8, want as u8)
                .unwrap_or_else(|| fatal(13));
            seq = lease.sequence;
            s.leases[cpu] = Some(lease);
            target = want;
            if native_el0_mmu::active() != Some(want) {
                unsafe {
                    native_el0_mmu::switch(want).unwrap_or_else(|_| fatal(14));
                }
            }
            unsafe {
                INCOMING[cpu] = s.frames[want];
            }
        }
        let mut send = 0u8;
        for c in 0..CPUS {
            if c == cpu {
                continue;
            }
            let running = s.leases[c].map_or(IDLE, |l| l.task as usize);
            let ready = proposed[c] != IDLE
                && !s
                    .leases
                    .iter()
                    .flatten()
                    .any(|l| l.task as usize == proposed[c]);
            if running != proposed[c] && (running != IDLE || ready) && s.pending & (1 << c) == 0 {
                send |= 1 << c;
            }
        }
        s.pending |= send;
        let prior_compare = s.compare[cpu];
        let epoch = E.load(Acquire);
        let grid = if now < epoch {
            0
        } else {
            add((now - epoch) / q(), 1)
        };
        let mut next = due(grid);
        for j in s.job {
            if let Job::Sleep(w) = j {
                next = next.min(w);
            }
        }
        s.compare[cpu] = next;
        let mut stop = 0;
        if cpu == 0 && s.exited == ALL && s.pending == 0 && !s.stopping {
            s.stopping = true;
            stop = ONLINE & !1;
        }
        let halt = s.stopping && cpu != 0 && id == Some(2);
        if id == Some(2) {
            let sender = (irq.unwrap() >> 10) & 7;
            if !s.stopping || (cpu == 0) == (sender == 0) {
                fatal(16);
            }
            if cpu == 0 {
                if PARKED.load(Acquire) & (1 << sender) == 0 {
                    fatal(16);
                }
                s.stop_acks |= 1 << sender;
            }
        }
        let end = s.stopping && cpu == 0 && s.stop_acks == ONLINE & !1;
        let i = s.used[cpu];
        if i < CAP {
            let pack = |a: [usize; CPUS]| {
                a.iter()
                    .enumerate()
                    .fold(0u64, |v, (c, &p)| v | ((p as u64) << (c * 4)))
            };
            let row = [
                s.revision,
                cpu as u64,
                i as u64,
                if irq.is_some() { 1 } else { 2 },
                op,
                entry,
                now,
                src as u64,
                target as u64,
                outgoing.map_or(0, |l| l.sequence),
                seq,
                r,
                delta,
                if src == IDLE {
                    0
                } else {
                    s.owners.charged(src as u8).unwrap()
                },
                prior,
                root(),
                prior_compare,
                next,
                actual,
                ctl,
                releases as u64,
                wakes as u64,
                pack(old),
                pack(proposed),
                send as u64,
                s.pending as u64,
                stop as u64,
                s.exited as u64,
                handler_sp,
                saved.x[0],
                verdict,
                esr,
                grid,
                bsp::idle_stamp(cpu),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            unsafe {
                native_mig_events[cpu][i] = row;
                native_mig_frames[cpu][i] = [
                    saved,
                    if target == IDLE {
                        Frame::ZERO
                    } else {
                        INCOMING[cpu]
                    },
                ];
            }
            s.used[cpu] += 1;
        } else {
            s.dropped[cpu] = add(s.dropped[cpu], 1);
        }
        Transfer {
            index: i,
            target,
            send,
            stop,
            park: halt,
            end,
            next,
        }
    });
    let stamp = |field: usize, value: u64| {
        if t.index < CAP {
            // Sole CPU writer, masked until BSP restore; terminal export acquires all park acknowledgments.
            unsafe {
                native_mig_events[cpu][t.index][field] = value;
            }
        }
    };
    if let Some(iar) = irq {
        if iar & 1023 < 1020 {
            bsp::eoi(iar);
            stamp(34, counter());
        }
    }
    for c in 0..CPUS {
        if t.send & (1 << c) != 0 {
            stamp(35 + 2 * c, counter());
            bsp::send_sgi(c, 1);
            stamp(36 + 2 * c, counter());
        }
        if t.stop & (1 << c) != 0 {
            bsp::send_sgi(c, 2);
        }
    }
    if t.park {
        bsp::disable_timer();
        PARKED.fetch_or(1 << cpu, Release);
        bsp::send_sgi(0, 2);
        park();
    }
    if t.end {
        bsp::disable_timer();
        finish();
    }
    bsp::arm(t.next);
    let (cval, control): (u64, u64);
    unsafe {
        asm!("isb","mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0",out(reg)cval,out(reg)control,options(nostack,preserves_flags));
    }
    stamp(41, cval);
    stamp(42, control);
    stamp(43, counter());
    if t.target == IDLE {
        ptr::null()
    } else {
        unsafe { &raw const INCOMING[cpu] }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_irq(
    cpu: usize,
    fp: *mut Frame,
    entry: u64,
    iar: u32,
) -> *const Frame {
    dispatch(cpu, fp, entry, Some(iar), 0)
}
#[unsafe(no_mangle)]
extern "C" fn native_partitioned_sync(
    cpu: usize,
    fp: *mut Frame,
    entry: u64,
    esr: u64,
) -> *const Frame {
    dispatch(cpu, fp, entry, None, esr)
}
fn finish() -> ! {
    // Every secondary has published PARKED and stopped accessing shared data.
    let (used, lost, jobs, charges) = with_state(|slot| {
        let s = slot.as_ref().unwrap();
        (
            s.used,
            s.dropped,
            s.job,
            core::array::from_fn::<_, COUNT, _>(|p| s.owners.charged(p as u8).unwrap()),
        )
    });
    print(
        b"CONFIG",
        &[
            1,
            HZ.load(Acquire),
            q(),
            E.load(Acquire),
            CPUS as u64,
            COUNT as u64,
        ],
    );
    for c in 0..CPUS {
        print(
            b"CAL",
            &[
                c as u64,
                BEFORE.load(Relaxed),
                CAL[c].load(Relaxed),
                AFTER.load(Relaxed),
            ],
        );
        print(b"STACK", &bsp::stack_state(c));
        print(
            b"JOURNAL",
            &[
                c as u64,
                unsafe { (&raw const native_mig_events[c]).addr() as u64 },
                used[c] as u64,
                W as u64,
                lost[c],
                unsafe { (&raw const native_mig_frames[c]).addr() as u64 },
                core::mem::size_of::<Frame>() as u64,
            ],
        );
    }
    for p in 0..COUNT {
        let Job::Exit(status) = jobs[p] else {
            fatal(15)
        };
        print(
            b"TASK",
            &[
                p as u64,
                NATIVE_SMP_CPU_MASKS[p] as u64,
                NATIVE_SMP_EFFECTIVE_MASKS[p] as u64,
                NATIVE_PRIORITIES[p] as u64,
                NATIVE_SMP_RELEASE_CELLS[p],
                charges[p],
                native_el0_mmu::space_id(p).unwrap(),
            ],
        );
        uart_write(b"NATIVE EL0 EXIT PID ");
        unsigned(p as u64);
        uart_write(b" ");
        decimal(i64::from(status));
        uart_write(b"\n");
    }
    head(10, 1);
    head(1, 3);
    print(b"END", &[ALL as u64, PARKED.load(Acquire) as u64]);
    park()
}
