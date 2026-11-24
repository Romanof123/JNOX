use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};

use super::context::*;
use super::reservation_jobs::{
    Action, Clock, Error, FirstTransition, Jobs, Manifest, Policy, admission, charge,
};
use super::{park, uart_write};

const IDLE: usize = 2;
const NA: u64 = u64::MAX;
const SEED: [u64; 2] = [0xa19, 0xb19];
const MAX_ROWS: usize = 1024;
const LATE: bool = cfg!(jnox_reservation_case = "late");

global_asm!(".set JNOX_AP,1",include_str!("context.S"),frame_size=const size_of::<Frame>(),
    elr=const offset_of!(Frame,elr),fpcr=const offset_of!(Frame,fpcr),
    tpidr_el1=const offset_of!(Frame,tpidr_el1),tpidr_el0=const offset_of!(Frame,tpidr_el0),
    simd=const offset_of!(Frame,simd),irq=sym irq,svc=sym svc,fail=sym unexpected);

#[repr(C)]
struct Returns {
    count: u64,
    entries: [[u64; 3]; 64],
}
// Exact existing context.S pre-ERET recorder ABI. Mutually exclusive backend.
#[unsafe(no_mangle)]
static mut jnox_ap_returns: Returns = Returns {
    count: 0,
    entries: [[0; 3]; 64],
};
static mut PLANS: [[u64; 2]; 64] = [[NA; 2]; 64];
static mut TRACE: [[u64; 8]; MAX_ROWS] = [[0; 8]; MAX_ROWS];
static mut USED: usize = 0;
static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut RUNTIME: Option<Runtime> = None;
static mut TERMINAL: bool = false;

fn now() -> u64 {
    let value;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)value,options(nostack,preserves_flags)) };
    value
}
fn event(tag: u64, t: u64, slot: u64, id: usize, fields: [u64; 4]) {
    // Sole core, producer always boot/IRQ/SVC masked.
    unsafe {
        if USED == MAX_ROWS {
            uart_write(b"RSRV TRACE OVERFLOW\n");
            park()
        }
        (&raw mut TRACE).cast::<[u64; 8]>().add(USED).write([
            tag, t, slot, id as u64, fields[0], fields[1], fields[2], fields[3],
        ]);
        USED += 1;
    }
}
fn decimal(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = 20;
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
fn returned(i: usize) -> [u64; 3] {
    unsafe {
        (&raw const jnox_ap_returns.entries)
            .cast::<[u64; 3]>()
            .add(i)
            .read_volatile()
    }
}
fn return_count() -> usize {
    unsafe { (&raw const jnox_ap_returns.count).read_volatile() as usize }
}
fn finish(status: u64, why: u64, slot: u64, id: usize, a: u64, b: u64) -> ! {
    unsafe {
        asm!(
            "msr daifset,#0xf",
            "msr cntp_ctl_el0,xzr",
            "isb",
            options(nostack)
        );
        if TERMINAL {
            park()
        }
        TERMINAL = true;
    }
    for i in 0..return_count().min(64) {
        let [frame, pc, t] = returned(i);
        let [s, owner] = unsafe { PLANS[i] };
        event(12, t, s, owner as usize, [frame, pc, i as u64, 0]);
    }
    event(13, now(), slot, id, [status, why, a, b]);
    uart_write(b"RSRV RAW 1\n");
    for i in 0..unsafe { USED } {
        let row = unsafe { (&raw const TRACE).cast::<[u64; 8]>().add(i).read() };
        for n in row {
            decimal(n);
            uart_write(b" ")
        }
        uart_write(b"\n")
    }
    uart_write(b"RSRV END\n");
    park()
}
extern "C" fn unexpected() -> ! {
    finish(3, 1, NA, IDLE, 0, 0)
}
fn checked<T>(r: Result<T, Error>) -> T {
    r.unwrap_or_else(|e| finish(3, 2, NA, IDLE, e as u64, 0))
}
fn state() -> &'static mut Runtime {
    unsafe {
        (*(&raw mut RUNTIME))
            .as_mut()
            .unwrap_or_else(|| unexpected())
    }
}
fn top(id: usize) -> usize {
    (&raw mut STACKS).addr() + (id + 1) * STACK_BYTES
}
fn within(id: usize, address: usize, bytes: usize) -> bool {
    let hi = top(id);
    address >= hi - STACK_BYTES && address.checked_add(bytes).is_some_and(|end| end <= hi)
}
fn fresh(id: usize) -> *mut Frame {
    let entry = [
        reservation_worker_a as *const (),
        reservation_worker_b as *const (),
        idle as *const (),
    ][id];
    let frame = (top(id) - size_of::<Frame>()) as *mut Frame;
    unsafe {
        frame.write_bytes(0, 1);
        (*frame).elr = entry as usize as u64;
        (*frame).spsr = 0x345;
        (*frame).tpidr_el1 = if id < IDLE { SEED[id] } else { 0xc19 };
    }
    frame
}
extern "C" fn idle() -> ! {
    if !LATE {
        // The first opening timer is already armed. If setup crosses E,
        // retain that epoch and report the missed preparation explicitly.
        unsafe { asm!("msr daifset,#2", "isb", options(nostack)) };
        state().prepare_first();
        unsafe { asm!("msr daifclr,#2", "isb", options(nostack)) };
    }
    loop {
        unsafe { asm!("wfi", options(nostack)) }
    }
}

// A worker publishes one complete 16-update chunk at a time. The private
// stack has two cells so an asynchronously interrupted partial write never
// combines a new count with a prior checksum. The IRQ never writes these cells.
struct Progress {
    cells: [[AtomicU64; 2]; 2],
    active: AtomicUsize,
}
static POINTER: [AtomicPtr<Progress>; 2] = [const { AtomicPtr::new(ptr::null_mut()) }; 2];
static GOAL: [AtomicU64; 2] = [const { AtomicU64::new(NA) }; 2];
static ACTION: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static GENERATION: [AtomicU64; 2] = [const { AtomicU64::new(NA) }; 2];
static FIRST: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static MASK_FROM: AtomicU64 = AtomicU64::new(NA);
static MASK_UNTIL: AtomicU64 = AtomicU64::new(NA);
static NOMINAL_END: AtomicU64 = AtomicU64::new(NA);
static EDGE_TICK: AtomicU64 = AtomicU64::new(0);
static EDGE_COUNT: AtomicU64 = AtomicU64::new(0);
static END_COUNT: AtomicU64 = AtomicU64::new(0);
static MASK_BEGIN: AtomicU64 = AtomicU64::new(0);
static MASK_END: AtomicU64 = AtomicU64::new(0);

#[inline(never)]
fn virtual_mask_hook(committed: u64) {
    // Only the late image executes this diagnostic. No spin/delay loop in
    // the harness: the worker keeps doing its original chunks throughout.
    if MASK_BEGIN.load(Ordering::Relaxed) == 0 {
        if now() >= MASK_FROM.load(Ordering::Relaxed) {
            unsafe { asm!("msr daifset,#2", "isb", options(nostack)) };
            MASK_BEGIN.store(now(), Ordering::Release);
        }
    } else if MASK_END.load(Ordering::Relaxed) == 0 {
        let tick = now();
        if tick >= NOMINAL_END.load(Ordering::Relaxed) && EDGE_TICK.load(Ordering::Relaxed) == 0 {
            EDGE_COUNT.store(committed, Ordering::Relaxed);
            EDGE_TICK.store(tick, Ordering::Release);
        }
        if tick >= MASK_UNTIL.load(Ordering::Relaxed) {
            END_COUNT.store(committed, Ordering::Relaxed);
            MASK_END.store(now(), Ordering::Release);
            unsafe { asm!("msr daifclr,#2", "isb", options(nostack)) }
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn reservation_worker_a() -> ! {
    reservation_rust_work(0)
}
#[unsafe(no_mangle)]
extern "C" fn reservation_worker_b() -> ! {
    reservation_rust_work(1)
}
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn reservation_rust_work(id: usize) -> ! {
    let mut n = 0u64;
    let mut acc = SEED[id];
    let mut progress = Progress {
        cells: core::array::from_fn(|_| [AtomicU64::new(0), AtomicU64::new(acc)]),
        active: AtomicUsize::new(0),
    };
    POINTER[id].store(&raw mut progress, Ordering::Release);
    let mut observed = NA;
    loop {
        let generation = GENERATION[id].load(Ordering::Acquire);
        if generation != observed {
            FIRST[id].store(now(), Ordering::Release);
            observed = generation;
        }
        if LATE && id == 0 {
            virtual_mask_hook(n)
        }
        for _ in 0..16 {
            acc = core::hint::black_box(acc.wrapping_mul(3).wrapping_add(1));
        }
        n = n.wrapping_add(16);
        let index = 1 - progress.active.load(Ordering::Relaxed);
        progress.cells[index][0].store(n, Ordering::Relaxed);
        progress.cells[index][1].store(acc, Ordering::Relaxed);
        progress.active.store(index, Ordering::Release);
        if n == GOAL[id].load(Ordering::Acquire) {
            if ACTION[id].load(Ordering::Relaxed) == 2 {
                unsafe { asm!("svc #1", options(nostack)) }
            } else {
                unsafe { asm!("svc #2", options(noreturn)) }
            }
        }
    }
}
// Compose the affine update by squaring in at most 64 iterations. Grant
// accounting uses the already captured vector timestamp, not this monitor.
fn advanced(mut x: u64, mut steps: u64) -> u64 {
    let (mut a, mut b) = (3u64, 1u64);
    while steps != 0 {
        if steps & 1 != 0 {
            x = a.wrapping_mul(x).wrapping_add(b)
        }
        b = a.wrapping_mul(b).wrapping_add(b);
        a = a.wrapping_mul(a);
        steps >>= 1;
    }
    x
}
fn snapshot(id: usize) -> (u64, u64, usize) {
    let p = POINTER[id].load(Ordering::Acquire);
    if p.is_null() {
        return (0, SEED[id], 0);
    }
    if !within(id, p.addr(), size_of::<Progress>()) {
        unexpected()
    }
    let p = unsafe { &*p };
    let index = p.active.load(Ordering::Acquire);
    (
        p.cells[index][0].load(Ordering::Relaxed),
        p.cells[index][1].load(Ordering::Relaxed),
        p as *const _ as usize,
    )
}

#[derive(Clone, Copy)]
struct Grant {
    id: usize,
    action: Action,
    slot: u64,
    ret: usize,
    count: u64,
    acc: u64,
    deadline: u64,
}
struct Prepared {
    setup_start: u64,
    transition: FirstTransition,
    epoch: u64,
    end: u64,
    cutoff: u64,
    goal: u64,
    frames: [usize; 2],
}

struct Runtime {
    jobs: Jobs,
    prepared: Option<Prepared>,
    clock: Clock,
    horizon: u64,
    opened: u64,
    current: usize,
    frames: [*mut Frame; 3],
    grant: Option<Grant>,
    target: u64,
    kind: u64,
    allocation_deadline: [u64; 2],
    allocation_service: [u64; 2],
    capacity: [u64; 2],
    last_count: [u64; 2],
    last_acc: [u64; 2],
}

impl Runtime {
    fn first_frames(&self) -> [usize; 2] {
        core::array::from_fn(|id| {
            let frame = self.frames[id];
            if frame.addr() != top(id) - size_of::<Frame>()
                || !within(id, frame.addr(), size_of::<Frame>())
            {
                unexpected()
            }
            let f = unsafe { &*frame };
            let pc = [
                reservation_worker_a as *const (),
                reservation_worker_b as *const (),
            ][id];
            if f.elr != pc as usize as u64
                || f.spsr != 0x345
                || f.gpr[19] != 0
                || f.tpidr_el1 != SEED[id]
                || GENERATION[id].load(Ordering::Acquire) != NA
                || ACTION[id].load(Ordering::Acquire) != 0
                || GOAL[id].load(Ordering::Acquire) != NA
                || FIRST[id].load(Ordering::Acquire) != 0
                || !POINTER[id].load(Ordering::Acquire).is_null()
                || self.last_count[id] != 0
                || self.last_acc[id] != SEED[id]
                || self.allocation_deadline[id] != NA
                || self.allocation_service[id] != 0
            {
                unexpected()
            }
            frame.addr()
        })
    }

    fn prepare_first(&mut self) {
        let begin = now();
        let epoch = self.clock.epoch;
        if self.prepared.is_some()
            || self.opened != NA
            || self.current != IDLE
            || self.grant.is_some()
            || self.kind != 0
            || self.target != epoch
            || return_count() != 1
        {
            unexpected()
        }
        let transition = checked(self.jobs.prepare_first());
        let decision = transition.decision;
        let id = decision.owner.unwrap_or_else(|| unexpected());
        let action = decision.action.unwrap_or_else(|| unexpected());
        let goal = if action == Action::Long {
            NA
        } else {
            self.last_count[id]
                .checked_add(256)
                .unwrap_or_else(|| unexpected())
        };
        let p = Prepared {
            setup_start: begin,
            transition,
            epoch,
            goal,
            end: checked(self.clock.boundary(1)),
            cutoff: checked(self.clock.cutoff(0)),
            frames: self.first_frames(),
        };
        self.prepared = Some(p);
        let done = now();
        event(
            22,
            done,
            0,
            IDLE,
            [begin, epoch, epoch.saturating_sub(done), 0],
        );
        if done < begin || done >= epoch {
            finish(2, 9, 0, IDLE, done, epoch)
        }
    }

    fn arm(&mut self, slot: u64, kind: u64, target: u64) {
        self.target = target;
        self.kind = kind;
        let (compare, ctl): (u64, u64);
        unsafe {
            asm!("msr cntp_cval_el0,{target}","msr cntp_ctl_el0,{one}","isb",
            "mrs {compare},cntp_cval_el0","mrs {ctl},cntp_ctl_el0",
            target=in(reg)target,one=in(reg)1u64,compare=out(reg)compare,ctl=out(reg)ctl,options(nostack))
        };
        event(3, now(), slot, self.current, [target, ctl, compare, kind]);
        if compare != target || ctl & 3 != 1 {
            unexpected()
        }
    }
    fn select(&mut self, slot: u64, id: usize) -> *mut Frame {
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        if !within(id, frame.addr(), size_of::<Frame>()) {
            unexpected()
        }
        let i = return_count();
        if i >= 64 {
            unexpected()
        }
        let saved = unsafe { &*frame };
        event(
            18,
            now(),
            slot,
            id,
            [
                frame.addr() as u64,
                saved.elr,
                saved.gpr[19],
                saved.tpidr_el1,
            ],
        );
        unsafe { PLANS[i] = [slot, id as u64] };
        self.current = id;
        frame
    }
    fn open(&mut self, slot: u64, entry: u64) -> *mut Frame {
        // After an early retirement or cutoff only idle may have run. Verify
        // both clients at the boundary, including final idle and exited clients.
        for id in 0..IDLE {
            let (count, acc, ptr) = snapshot(id);
            if slot == self.horizon {
                event(21, now(), slot, id, [count, acc, ptr as u64, 0])
            }
            if count != self.last_count[id] || acc != self.last_acc[id] {
                finish(3, 8, slot, id, count, acc)
            }
        }
        if slot == self.horizon {
            finish(0, 0, slot, IDLE, 0, 0)
        }
        let (d, start, end, prepared_grant) = if slot == 0 && !LATE {
            let p = self.prepared.take().unwrap_or_else(|| unexpected());
            let ret = (return_count() == 1).then(|| returned(0));
            let idle_entry = idle as *const () as usize as u64;
            if self.opened != NA
                || self.current != IDLE
                || self.grant.is_some()
                || self.kind != 0
                || self.target != p.epoch
                || self.clock.epoch != p.epoch
                || !super::reservation_jobs::first_return_matches(
                    ret,
                    (top(IDLE) - size_of::<Frame>()) as u64,
                    idle_entry,
                    p.setup_start,
                    p.frames,
                    self.first_frames(),
                )
            {
                unexpected()
            }
            let before = now();
            if before < entry || !self.clock.within_opening(before) {
                finish(2, 5, 0, IDLE, before, p.epoch)
            }
            let decision = checked(self.jobs.commit_first(p.transition));
            let committed = now();
            event(
                23,
                committed,
                0,
                decision.owner.unwrap_or(IDLE),
                [entry, before, p.epoch, 1],
            );
            if committed < before || !self.clock.within_opening(committed) {
                finish(2, 5, 0, IDLE, committed, p.epoch)
            }
            (decision, p.epoch, p.end, Some((p.goal, p.cutoff)))
        } else {
            (
                checked(self.jobs.open(slot)),
                checked(self.clock.boundary(slot)),
                checked(self.clock.boundary(slot + 1)),
                None,
            )
        };
        self.opened = slot;
        for id in 0..2 {
            event(
                5,
                now(),
                slot,
                id,
                [d.before[id].q, d.before[id].d, d.after[id].q, d.after[id].d],
            );
            let action = if Some(id) == d.owner {
                d.action.map_or(0, |a| 1 + a as u64)
            } else {
                0
            };
            event(
                6,
                now(),
                slot,
                id,
                [
                    u64::from(d.releases[id]),
                    u64::from(d.demand[id]),
                    d.states[id] as u64,
                    action,
                ],
            );
        }
        if let Some(id) = d.owner {
            let action = d.action.unwrap_or_else(|| unexpected());
            let (count, acc, _) = snapshot(id);
            if self.allocation_deadline[id] != d.after[id].d {
                self.allocation_deadline[id] = d.after[id].d;
                self.allocation_service[id] = 0;
            }
            self.grant = Some(Grant {
                id,
                action,
                slot,
                ret: return_count(),
                count,
                acc,
                deadline: d.after[id].d,
            });
            GOAL[id].store(
                if let Some((goal, _)) = prepared_grant {
                    goal
                } else if action == Action::Long {
                    NA
                } else {
                    count.checked_add(256).unwrap_or_else(|| unexpected())
                },
                Ordering::Release,
            );
            ACTION[id].store(1 + action as u64, Ordering::Relaxed);
            FIRST[id].store(0, Ordering::Relaxed);
            GENERATION[id].store(slot, Ordering::Release);
            let cutoff = prepared_grant
                .map_or_else(|| checked(self.clock.cutoff(slot)), |(_, cutoff)| cutoff);
            if LATE && slot == 0 {
                MASK_FROM.store(cutoff - self.clock.millisecond, Ordering::Relaxed);
                NOMINAL_END.store(end, Ordering::Relaxed);
                MASK_UNTIL.store(end + 2 * self.clock.millisecond, Ordering::Relaxed);
            }
            self.arm(slot, 1, cutoff);
        } else {
            self.arm(slot, 0, end)
        }
        let id = d.owner.unwrap_or(IDLE);
        let frame = self.select(slot, id);
        event(
            7,
            now(),
            slot,
            id,
            [start, end, frame.addr() as u64, unsafe { (*frame).elr }],
        );
        frame
    }
    fn close(&mut self, frame: *mut Frame, entry: u64, action: Action, syndrome: u64) {
        let grant = self.grant.take().unwrap_or_else(|| unexpected());
        let id = grant.id;
        let slot = grant.slot;
        if self.current != id || !within(id, frame.addr(), size_of::<Frame>()) {
            unexpected()
        }
        let saved = unsafe { &*frame };
        let (count, acc, p) = snapshot(id);
        let first = FIRST[id].load(Ordering::Acquire);
        let start = checked(self.clock.boundary(slot));
        let end = checked(self.clock.boundary(slot + 1));
        let cutoff = checked(self.clock.cutoff(slot));
        if return_count() != grant.ret + 1 {
            unexpected()
        }
        let [rf, rpc, lower] = returned(grant.ret);
        let upper = entry;
        let elapsed = upper.checked_sub(lower).unwrap_or_else(|| unexpected());
        let total = checked(charge(self.allocation_service[id], lower, upper));
        self.allocation_service[id] = total;
        event(8, entry, slot, id, [grant.count, count, grant.acc, acc]);
        event(
            9,
            entry,
            slot,
            id,
            [
                frame.addr() as u64,
                saved.elr,
                saved.gpr[19],
                saved.tpidr_el1,
            ],
        );
        event(10, entry, slot, id, [lower, first, upper, elapsed]);
        event(
            11,
            entry,
            slot,
            id,
            [
                1 + action as u64,
                syndrome,
                p as u64,
                (frame.addr() + size_of::<Frame>()) as u64,
            ],
        );
        event(19, entry, slot, id, [grant.deadline, total, rf, rpc]);
        let begin = MASK_BEGIN.load(Ordering::Acquire);
        let unmask = MASK_END.load(Ordering::Acquire);
        if LATE {
            let edge_tick = EDGE_TICK.load(Ordering::Acquire);
            let edge_count = EDGE_COUNT.load(Ordering::Relaxed);
            let end_count = END_COUNT.load(Ordering::Relaxed);
            event(16, entry, slot, id, [begin, unmask, cutoff, end]);
            event(
                20,
                entry,
                slot,
                id,
                [edge_tick, edge_count, unmask, end_count],
            );
        }
        if LATE && slot == 0 && entry > end {
            event(14, entry, slot, id, [end, entry, 1, entry - end]);
        }
        if action != grant.action
            || count <= grant.count
            || saved.tpidr_el1 != SEED[id]
            || saved.spsr & 0x8f != 5
            || (count > grant.count && advanced(grant.acc, count - grant.count) != acc)
            || (action != Action::Long && count - grant.count != 256)
        {
            finish(3, 3, slot, id, count, grant.count)
        }
        if lower < start || first < lower || first > upper || first - start > self.clock.millisecond
        {
            finish(2, 2, slot, id, first, start)
        }
        if LATE && slot == 0 {
            let edge = EDGE_TICK.load(Ordering::Acquire);
            let mask_ok = begin > 0
                && begin < cutoff
                && end <= edge
                && edge < unmask
                && unmask >= MASK_UNTIL.load(Ordering::Relaxed)
                && unmask <= upper
                && END_COUNT.load(Ordering::Relaxed) > EDGE_COUNT.load(Ordering::Relaxed);
            if !mask_ok {
                finish(3, 10, slot, id, begin, unmask)
            }
        }
        if upper > end
            || (action == Action::Long && entry.saturating_sub(cutoff) > self.clock.millisecond)
        {
            finish(2, 1, slot, id, upper, end)
        }
        if elapsed > self.clock.slot_ticks || total > self.capacity[id] * self.clock.slot_ticks {
            finish(2, 7, slot, id, elapsed, total)
        }
        if action != Action::Long && entry >= cutoff {
            finish(2, 9, slot, id, entry, cutoff)
        }
        self.last_count[id] = count;
        self.last_acc[id] = acc;
        checked(self.jobs.trap(id, action));
        if action != Action::Exit {
            self.frames[id] = frame;
        }
    }
}
extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let s = state();
    if !within(s.current, frame.addr(), size_of::<Frame>()) {
        unexpected()
    }
    let entry = unsafe { (*frame).reserved };
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {},cntp_ctl_el0",out(reg)ctl,options(nostack)) };
    let slot = if s.kind == 1 {
        s.opened
    } else if s.opened == NA {
        0
    } else {
        s.opened + 1
    };
    event(
        4,
        entry,
        slot,
        s.current,
        [s.target, s.kind, iar as u64, ctl],
    );
    if slot == 0 && s.kind == 0 && !LATE {
        let compare: u64;
        unsafe { asm!("mrs {},cntp_cval_el0",out(reg)compare,options(nostack)) };
        if compare != s.target || compare != s.clock.epoch {
            unexpected()
        }
    }
    if iar & 0x3ff != TIMER_IRQ || ctl & 7 != 5 || entry < s.target {
        finish(3, 4, slot, s.current, iar as u64, ctl)
    }
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", "dsb sy", options(nostack)) };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack)) };
    if s.kind == 1 {
        s.close(frame, entry, Action::Long, 0);
        s.arm(slot, 0, checked(s.clock.boundary(slot + 1)));
        s.select(slot, IDLE)
    } else {
        if s.current != IDLE || s.grant.is_some() {
            unexpected()
        }
        s.frames[IDLE] = frame;
        if entry - s.target > s.clock.millisecond {
            finish(2, 5, slot, IDLE, entry, s.target)
        }
        s.open(slot, entry)
    }
}
extern "C" fn svc(frame: *mut Frame) -> *mut Frame {
    let s = state();
    if s.current == IDLE || !within(s.current, frame.addr(), size_of::<Frame>()) {
        unexpected()
    }
    let entry = unsafe { (*frame).reserved };
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1",out(reg)esr,options(nostack)) };
    if esr >> 26 != 0x15 {
        unexpected()
    }
    let action = match esr & 0xffff {
        1 => Action::Block,
        2 => Action::Exit,
        _ => unexpected(),
    };
    s.close(frame, entry, action, esr);
    s.arm(s.opened, 0, checked(s.clock.boundary(s.opened + 1)));
    s.select(s.opened, IDLE)
}

pub(super) fn run() -> ! {
    #[cfg(any(jnox_reservation_case = "wake", jnox_reservation_case = "saturated"))]
    let boot_sample = now();
    let policy = if cfg!(jnox_reservation_policy = "cbs") {
        Policy::Cbs
    } else {
        Policy::Aligned
    };
    let (manifest, case) = if cfg!(jnox_reservation_case = "refused") {
        (Manifest::R, 2)
    } else if cfg!(jnox_reservation_case = "wake") {
        (Manifest::W, 0)
    } else if LATE {
        (Manifest::S, 3)
    } else {
        (Manifest::S, 1)
    };
    let hz: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)hz,options(nostack)) };
    let epoch = now().checked_add(hz / 20).unwrap_or_else(|| unexpected());
    let clock = checked(Clock::new(epoch, hz, manifest));
    event(
        1,
        now(),
        NA,
        IDLE,
        [hz, clock.slot_ticks, policy as u64, case],
    );
    if manifest == Manifest::R {
        let over = Jobs::new(policy, manifest).err();
        let controls = [
            (over, Error::OverCapacity),
            (admission(&[(0, 4)]).err(), Error::Invalid),
            (admission(&[(3, 2)]).err(), Error::Invalid),
            (
                admission(&[(1, 4294967291), (1, 4294967279), (1, 4294967231)]).err(),
                Error::Overflow,
            ),
            (admission(&[(1, 0)]).err(), Error::Invalid),
        ];
        for (i, (actual, expected)) in controls.into_iter().enumerate() {
            event(
                15,
                now(),
                NA,
                IDLE,
                [i as u64, actual.map_or(0, |e| e as u64), 0, 0],
            );
            if actual != Some(expected) {
                finish(3, 6, NA, IDLE, i as u64, expected as u64)
            }
        }
        finish(1, Error::OverCapacity as u64, NA, IDLE, 9, 8)
    }
    let jobs = checked(Jobs::new(policy, manifest));
    #[cfg(any(jnox_reservation_case = "wake", jnox_reservation_case = "saturated"))]
    event(24, now(), NA, IDLE, [boot_sample, epoch, 0, 0]);
    init_timer_irq();
    let frames = core::array::from_fn(fresh);
    let horizon = manifest.horizon();
    for (id, _) in frames.iter().enumerate() {
        event(
            17,
            now(),
            NA,
            id,
            [
                (top(id) - STACK_BYTES) as u64,
                top(id) as u64,
                if id < IDLE { SEED[id] } else { 0xc19 },
                0,
            ],
        );
    }
    unsafe {
        RUNTIME = Some(Runtime {
            jobs,
            prepared: None,
            clock,
            horizon,
            opened: NA,
            current: IDLE,
            frames,
            grant: None,
            target: 0,
            kind: 0,
            allocation_deadline: [NA; 2],
            allocation_service: [0; 2],
            last_count: [0; 2],
            last_acc: SEED,
            capacity: if manifest == Manifest::W {
                [2, 1]
            } else {
                [1, 1]
            },
        })
    };
    event(2, now(), NA, IDLE, [epoch, horizon, 0, 0]);
    let s = state();
    s.arm(0, 0, epoch);
    let frame = s.select(NA, IDLE);
    unsafe { sched_start_frame(frame) }
}
