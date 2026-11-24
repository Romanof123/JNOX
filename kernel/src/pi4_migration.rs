//! One-lock, two-CPU ordinary migration case. All scheduler mutations run on
//! masked boot/dispatcher stacks. A worker never holds the scheduler lock.
use crate::{
    context::{Frame, STACK_BYTES, Stack, sched_start_frame},
    pi4_migration_log as log, pi4_migration_tasks as jobs, pi4_smp_bsp as bsp,
    smp_policy::{Disposition, Lease, Ownership},
};
use core::{
    arch::asm,
    cell::UnsafeCell,
    mem::size_of,
    ptr,
    sync::atomic::{
        AtomicBool, AtomicU64,
        Ordering::{Acquire, Relaxed, Release},
    },
};

const IDLE: usize = 3;
const GUARD: u64 = 0x4d4947525354414b;
const APP: &str = env!("JNOX_MIGRATION_APP");
fn case() -> u64 {
    match APP {
        "migration-global" => 0,
        "migration-partitioned" => 1,
        "migration-clustered" => 2,
        _ => abort(1),
    }
}
fn masks() -> [u8; 3] {
    let cpu_clusters = [0, 1];
    let migrant = match case() {
        0 => 3,
        1 => 1,
        _ => cpu_clusters
            .iter()
            .enumerate()
            .fold(0, |mask, (cpu, cluster)| {
                mask | (u8::from(*cluster == cpu_clusters[0]) << cpu)
            }),
    };
    [migrant, 1, 2]
}
fn cpu() -> usize {
    (bsp::identity().0 & 0xff) as usize
}
fn sp() -> u64 {
    let v;
    unsafe { asm!("mov {},sp",out(reg)v,options(nomem,nostack,preserves_flags)) }
    v
}
pub(crate) fn abort(code: u64) -> ! {
    unsafe { asm!("msr daifset,#15", options(nostack)) };
    FAILED.store(true, Release);
    if cpu() == 0 {
        crate::uart_write(b"PI4 MIG FAIL ");
        for shift in (0..16).rev() {
            crate::uart_write(&[b"0123456789abcdef"[((code >> (4 * shift)) & 15) as usize]]);
        }
        crate::uart_write(b"\n");
    }
    crate::park()
}
static FAILED: AtomicBool = AtomicBool::new(false);
static INITIAL: AtomicBool = AtomicBool::new(false);
static DONE: [AtomicBool; 2] = [const { AtomicBool::new(false) }; 2];
static LIVE: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static MILESTONE: AtomicBool = AtomicBool::new(false);
static TASK_LEASE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static mut STACKS: [Stack; 7] = [const { Stack([0; STACK_BYTES]) }; 7];
#[unsafe(no_mangle)]
static mut smp_handler_tops: [usize; 2] = [0; 2];
#[unsafe(no_mangle)]
static pi4_migration_published_return_ticks: [AtomicU64; 2] =
    [const { AtomicU64::new(0) }; 2];
fn bounds(role: usize) -> (usize, usize) {
    let base = (&raw mut STACKS).cast::<Stack>().wrapping_add(role) as usize;
    (base + 16, base + STACK_BYTES - 16)
}
fn canary(role: usize) -> bool {
    let (lo, hi) = bounds(role);
    unsafe {
        ptr::read_volatile((lo - 16) as *const u64) == GUARD
            && ptr::read_volatile(hi as *const u64) == GUARD
    }
}
fn frame_check(role: usize, f: *mut Frame) {
    let (lo, hi) = bounds(role);
    if !canary(role) || f.addr() % 16 != 0 || !(lo..=hi - size_of::<Frame>()).contains(&f.addr()) {
        abort(2)
    }
}
fn fresh(role: usize, entry: extern "C" fn() -> !) -> usize {
    let f = (bounds(role).1 - size_of::<Frame>()) as *mut Frame;
    unsafe {
        f.write_bytes(0, 1);
        (*f).elr = entry as usize as u64;
        (*f).spsr = 0x345;
    }
    f.addr()
}

struct State {
    policy: Ownership<3>,
    running: [Option<Lease>; 2],
    frame: [usize; 3],
    epoch: u64,
    done: u8,
    summaries: u8,
}
struct Shared(UnsafeCell<Option<State>>);
unsafe impl Sync for Shared {}
static STATE: Shared = Shared(UnsafeCell::new(None));
static LOCK: AtomicBool = AtomicBool::new(false);
fn transaction<T>(f: impl FnOnce(&mut State) -> T) -> T {
    while LOCK
        .compare_exchange_weak(false, true, Acquire, Relaxed)
        .is_err()
    {
        core::hint::spin_loop()
    }
    let result = f(unsafe { (&mut *STATE.0.get()).as_mut().unwrap_or_else(|| abort(3)) });
    LOCK.store(false, Release);
    result
}
impl State {
    fn claim(&mut self, c: usize, id: u8) {
        let l = self.policy.claim(c as u8, id).unwrap_or_else(|| abort(4));
        let f = self.frame[id as usize] as *mut Frame;
        frame_check(id as usize, f);
        self.running[c] = Some(l);
        let frame = unsafe { &*f };
        log::event(
            cpu(),
            2,
            [
                self.epoch,
                id as u64,
                l.sequence,
                f.addr() as u64,
                frame.elr,
                frame.gpr[21],
                (f.addr() + size_of::<Frame>()) as u64,
                masks()[id as usize] as u64,
            ],
        );
    }
    fn dispatch(&self, c: usize) -> *mut Frame {
        let Some(l) = self.running[c] else { abort(5) };
        if l.cpu as usize != c {
            abort(6)
        }
        let id = l.task as usize;
        let f = self.frame[id] as *mut Frame;
        frame_check(id, f);
        let frame = unsafe { &*f };
        TASK_LEASE[id].store(l.sequence, Release);
        LIVE[c].store(l.sequence, Release);
        log::event(
            c,
            11,
            [
                self.epoch,
                id as u64,
                l.sequence,
                f.addr() as u64,
                frame.elr,
                (f.addr() + size_of::<Frame>()) as u64,
                frame.gpr[21],
                0,
            ],
        );
        f
    }
    fn retire(&mut self, c: usize, id: usize, stop: u64, ready: bool) {
        let l = self.running[c].take().unwrap_or_else(|| abort(7));
        if l.task as usize != id || LIVE[c].swap(0, Acquire) != l.sequence {
            abort(8)
        }
        let start = pi4_migration_published_return_ticks[c].load(Relaxed);
        let span = stop
            .checked_sub(start)
            .filter(|n| *n != 0 && start != 0)
            .unwrap_or_else(|| abort(9));
        if !self.policy.retire(
            l,
            span,
            if ready {
                Disposition::Ready
            } else {
                Disposition::Dormant
            },
        ) {
            abort(10)
        }
        log::event(
            c,
            3,
            [
                self.epoch,
                id as u64,
                l.sequence,
                start,
                stop,
                span,
                self.policy.charged(l.task).unwrap_or(0),
                u64::from(ready),
            ],
        );
    }
    fn place(&mut self, ready: u32, ranked: &[u8]) {
        let p = self.policy.plan(3, ranked).unwrap_or_else(|| abort(11));
        log::event(
            cpu(),
            6,
            [
                self.epoch,
                ready as u64,
                3,
                p[0].map_or(3, u64::from),
                p[1].map_or(3, u64::from),
                masks()[0] as u64,
                1,
                2,
            ],
        );
        for (c, id) in p[..2].iter().enumerate() {
            if let Some(id) = id {
                self.claim(c, *id)
            }
        }
    }
}
pub(crate) fn generation(task: usize) -> u64 {
    TASK_LEASE[task].load(Acquire)
}
pub(crate) fn first_milestone() -> bool {
    MILESTONE.load(Acquire)
}
// Dispatch restores one exclusively claimed frame. Only that running task
// holds this stack reference; handlers and the peer never receive it. The
// summary crosses SVC as scalar registers, and retirement precedes reuse.
#[derive(Default)]
pub(crate) struct Checkpoints {
    last_lease: u64,
    rows_on_lease: u64,
    count: u64,
    digest: u64,
    last_index: u64,
}

pub(crate) fn checkpoint(retained: &mut Checkpoints, before: u64, task: usize,
                         index: u64, sum: u64, local: usize) {
    #[cfg(not(feature = "pi4-stream-v1"))]
    if task == 0 && sum != index * (index + 1) / 2 {
        abort(12)
    }
    retained.count += 1;
    retained.digest = (retained.digest.rotate_left(7) ^ index).wrapping_add(sum);
    retained.last_index = index;
    let after = generation(task);
    if task != 0 || retained.last_lease != after || retained.rows_on_lease < 2
        || index == jobs::END || before != after
    {
        // Bind the sparse row and its CPU-local journal slot to the same lease.
        let daif: u64;
        unsafe { asm!("mrs {},daif","msr daifset,#2",out(reg)daif,options(nostack)) };
        let actual = cpu();
        let final_lease = LIVE[actual].load(Acquire);
        if final_lease == 0 || final_lease != generation(task) {
            abort(12)
        }
        if retained.last_lease != final_lease {
            retained.last_lease = final_lease;
            retained.rows_on_lease = 0;
        }
        retained.rows_on_lease += 1;
        let pc: u64;
        unsafe { asm!("adr {},.",out(reg)pc,options(nomem,nostack,preserves_flags)) }
        let stack = sp();
        log::event(
            actual,
            5,
            [
                task as u64,
                before,
                final_lease,
                index,
                sum,
                local as u64,
                pc,
                stack,
            ],
        );
        unsafe { asm!("msr daif,{}",in(reg)daif,options(nostack)) };
    }
    if task == 0 && index == jobs::BLOCK {
        MILESTONE.store(true, Release)
    }
}
pub(crate) fn finish(task: usize, value: u64, retained: Checkpoints) -> ! {
    unsafe { asm!("svc #1",in("x0")task,in("x1")value,in("x2")retained.count,
                  in("x3")retained.digest,in("x4")retained.last_index,options(noreturn)) }
}

fn idle_frame(c: usize, epoch: u64) -> *mut Frame {
    let f = fresh(3 + c, idle) as *mut Frame;
    log::event(
        c,
        11,
        [
            epoch,
            IDLE as u64,
            0,
            f.addr() as u64,
            unsafe { (*f).elr },
            (f.addr() + size_of::<Frame>()) as u64,
            0,
            0,
        ],
    );
    f
}
extern "C" fn idle() -> ! {
    let c = cpu();
    let begin = bsp::ticks();
    loop {
        if FAILED.load(Acquire)
            || bsp::ticks().wrapping_sub(begin) > bsp::frequency().saturating_mul(20)
        {
            abort(13)
        }
        if DONE[c].load(Acquire) {
            unsafe { asm!("msr daifset,#15", options(nostack)) };
            let epoch = transaction(|s| s.epoch);
            log::event(c, 10, [c as u64, epoch, 0, 0, 0, 0, 0, 0]);
            if !log::seal(c) {
                abort(14)
            }
            if c == 1 {
                SEALED.store(true, Release);
                crate::park()
            }
            while !SEALED.load(Acquire) {
                if FAILED.load(Acquire)
                    || bsp::ticks().wrapping_sub(begin) > bsp::frequency().saturating_mul(20)
                {
                    abort(15)
                }
            }
            let totals = transaction(|s| {
                if s.done != 7 || s.summaries != 15 || s.running.iter().any(Option::is_some) {
                    abort(27)
                }
                [0, 1, 2].map(|id| s.policy.charged(id).unwrap_or(0))
            });
            log::export(case(), bsp::frequency(), totals)
        }
        core::hint::spin_loop()
    }
}
static SEALED: AtomicBool = AtomicBool::new(false);
static HZ: AtomicU64 = AtomicU64::new(0);
pub(super) fn start(c: usize) -> ! {
    let identity = bsp::identity();
    let gic = bsp::gic_state(c);
    log::event(
        c,
        12,
        [
            identity.0,
            identity.1,
            bsp::frequency(),
            bsp::target(c) as u64,
            gic[0],
            gic[4],
            0,
            0,
        ],
    );
    if c == 0 {
        HZ.store(bsp::frequency(), Release);
        for role in 0..7 {
            let (lo, hi) = bounds(role);
            unsafe {
                ptr::write_volatile((lo - 16) as *mut u64, GUARD);
                ptr::write_volatile(hi as *mut u64, GUARD)
            }
            log::event(c, 1, [role as u64, lo as u64, hi as u64, 0, 0, 0, 0, 0]);
        }
        unsafe { smp_handler_tops = [bounds(5).1, bounds(6).1] };
        let mut s = State {
            policy: Ownership::new(masks()).unwrap_or_else(|| abort(16)),
            running: [None; 2],
            frame: [
                fresh(0, jobs::pi4_migration_m),
                fresh(1, jobs::pi4_migration_p0),
                fresh(2, jobs::pi4_migration_p1_open),
            ],
            epoch: 0,
            done: 0,
            summaries: 0,
        };
        if !s.policy.release(5) {
            abort(17)
        }
        s.place(5, &[0, 2]);
        unsafe { *STATE.0.get() = Some(s) };
        INITIAL.store(true, Release);
        if !bsp::release_cpu1() {
            abort(18)
        }
    } else if !INITIAL.load(Acquire) || HZ.load(Acquire) != bsp::frequency() {
        abort(19)
    }
    let f = transaction(|s| s.dispatch(c));
    if c == 0 {
        let start = bsp::ticks();
        // CPU1 publishes from its first frame restore, before P1 can run.
        while pi4_migration_published_return_ticks[1].load(Acquire) == 0 {
            if FAILED.load(Acquire)
                || bsp::ticks().wrapping_sub(start) > bsp::frequency().saturating_mul(4)
            {
                abort(28)
            }
        }
    }
    unsafe { sched_start_frame(f) }
}

fn trap(s: &State, c: usize, role: usize, f: *mut Frame, kind: u64) {
    frame_check(if role == IDLE { 3 + c } else { role }, f);
    let stack = sp();
    let (lo, hi) = bounds(5 + c);
    if !canary(5 + c) || !(lo..hi).contains(&(stack as usize)) {
        abort(20)
    }
    let fr = unsafe { &*f };
    log::event(
        c,
        4,
        [
            s.epoch,
            role as u64,
            f.addr() as u64,
            fr.elr,
            (f.addr() + size_of::<Frame>()) as u64,
            fr.gpr[21],
            kind,
            stack,
        ],
    );
}
pub(super) fn irq(c: usize, f: *mut Frame, iar: u32) -> *mut Frame {
    transaction(|s| {
        if c == 1 {
            trap(s, c, IDLE, f, iar as u64);
            if s.epoch != 1 || LIVE[c].load(Acquire) != 0 {
                abort(21)
            }
            return s.dispatch(c);
        }
        trap(s, c, 0, f, iar as u64);
        if s.epoch != 0 || s.running[1].is_some() || !first_milestone() {
            abort(22)
        }
        s.retire(c, 0, unsafe { (*f).reserved }, true);
        s.frame[0] = f.addr();
        s.epoch = 1;
        s.frame[2] = fresh(2, jobs::pi4_migration_p1);
        if !s.policy.release(6) {
            abort(23)
        }
        s.place(7, &[1, 0, 2]);
        let v = bsp::signal(1);
        log::event(c, 7, [1, v as u64, s.epoch, 0, 0, 0, 0, 0]);
        s.dispatch(c)
    })
}
pub(super) fn eoi_done(c: usize, iar: u32) {
    transaction(|s| log::event(c, 8, [iar as u64, s.epoch, 0, 0, 0, 0, 0, 0]));
}
pub(super) fn svc(c: usize, f: *mut Frame, imm: u16) -> *mut Frame {
    transaction(|s| {
        let l = s.running[c].unwrap_or_else(|| abort(24));
        let role = l.task as usize;
        trap(s, c, role, f, 0xffff);
        let fr = unsafe { &*f };
        if s.epoch == 0 && role == 0 && fr.gpr[0] == 0 && fr.gpr[1] == jobs::PRODUCT && imm == 1 {
            crate::uart_write(b"PI4 MIG UNOBSERVED M FINISHED BEFORE IRQ\n");
            abort(29)
        }
        let expected = match (s.epoch, role) {
            (0, 2) => 136,
            (1, 0) => jobs::PRODUCT,
            (1, 1) => 528,
            (1, 2) => 392,
            _ => abort(25),
        };
        if imm != 1 || fr.gpr[0] != role as u64 || fr.gpr[1] != expected {
            abort(26)
        }
        let (count, last_index, bit) = match (s.epoch, role) {
            (1, 0) => (jobs::END / jobs::BLOCK, jobs::END, 1),
            (1, 1) => (2, 32, 2),
            (0, 2) => (1, 16, 4),
            (1, 2) => (1, 32, 8),
            _ => abort(26),
        };
        if fr.gpr[2] != count || fr.gpr[4] != last_index || s.summaries & bit != 0 {
            abort(26)
        }
        s.summaries |= bit;
        log::terminal(
            c,
            [role as u64, s.epoch, l.sequence, fr.gpr[2],
             fr.gpr[3], fr.gpr[4], fr.gpr[1], 0],
            [role as u64, expected, l.sequence, s.epoch, 0, 0, 0, 0],
        );
        s.retire(c, role, fr.reserved, false);
        if s.epoch == 0 {
            let value = bsp::signal(0);
            log::event(c, 7, [0, value as u64, 0, 0, 0, 0, 0, 0]);
            return idle_frame(c, s.epoch);
        }
        s.done |= 1 << role;
        if role == 1 && case() != 0 {
            s.claim(c, 0);
            return s.dispatch(c);
        }
        if role == 0 && case() == 0 {
            s.claim(c, 2);
            return s.dispatch(c);
        }
        DONE[c].store(true, Release);
        idle_frame(c, s.epoch)
    })
}
