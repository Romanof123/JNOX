use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{Runtime, SENDER_US, app, fail, locked, log, now, state};

// A reviewed dynamic-bound and evidence package has not been supplied.
// Preserve the actual rejection before an operational timer or frame starts.
// Values are ticks at the declared architectural frequency, never observations
// promoted into WCET. No environment or UART field populates this table.
#[repr(C)]
struct Returns {
    count: u64,
    rows: [[u64; 3]; 128],
}
#[unsafe(no_mangle)]
static mut jnox_permit_returns: Returns = Returns {
    count: 0,
    rows: [[0; 3]; 128],
};
static BOUNDS: [u64; 12] = [0; 12];
#[derive(Clone, Copy)]
struct Margins {
    ready: u64,
    wake: u64,
    poll: u64,
    commit: u64,
    finish: u64,
    mask: u64,
}
static mut LIMITS: Option<Margins> = None;
static mut REQUEST: Option<u64> = None;
static mut LAST_WAKE: Option<u64> = None;
static mut WAKE: Option<u64> = None;
static mut ANCHOR: Option<u64> = None;
static mut ATTEMPT: u64 = 0;
static mut CONSUMED: u64 = 0;
static mut HALTED: bool = false;
static mut COMMAND_USED: bool = false;
static mut COMMAND_SEEN: bool = false;
static mut COMMAND_I: u64 = 0;
static mut COMMAND_U: u64 = 0;
static mut LAST_I: u64 = 0;
static mut LAST_U: u64 = 0;
#[derive(Clone, Copy)]
struct Payload {
    generation: u64,
    seq: u64,
    issued: u64,
}
static mut SLOT: Payload = Payload {
    generation: 0,
    seq: 0,
    issued: 0,
};
static COMMITTED: AtomicU64 = AtomicU64::new(0);
static COMMAND: AtomicU64 = AtomicU64::new(0);

pub(super) fn admit(hz: u64) -> bool {
    let v = unsafe { (&raw const BOUNDS).read_volatile() };
    log(64, now(), 1 + u64::from(cfg!(jnox_permit_anchor)), hz);
    // frequency, source evidence id, release-to-prepared, wake-to-worker,
    // final poll, commit, restore/SVC, mask allowance, receiver, ordinary
    // control base, supervisor, maximum masked/IRQ propagation.
    let [
        freq,
        evidence,
        ready,
        wake,
        poll,
        commit,
        finish,
        mask,
        receiver,
        control,
        supervisor,
        irq,
    ] = v;
    let mut missing = 0;
    for (i, n) in v.into_iter().enumerate() {
        if n == 0 {
            missing |= 1 << i
        }
    }
    let sum = |values: &[u64]| values.iter().try_fold(0u64, |a, &n| a.checked_add(n));
    let within = |ticks: Option<u64>, us: u64| {
        ticks.is_some_and(|t| u128::from(t) * 1_000_000 <= u128::from(hz) * u128::from(us))
    };
    let ok = missing == 0
        && evidence != 0
        && hz == freq
        && hz % 2000 == 0
        && within(sum(&[ready, wake, poll, commit, finish]), SENDER_US)
        && within(sum(&[receiver, control]), 250)
        && within(sum(&[irq, mask, supervisor]), 500)
        && within(sum(&[irq, mask, supervisor, receiver, control]), 1000)
        && commit <= mask
        && poll >= wake
        && within(sum(&[ready, poll]), 500);
    log(65, now(), u64::from(ok), missing);
    log(83, now(), u64::from(cfg!(jnox_permit_off)), evidence);
    #[cfg(jnox_permit_off)]
    {
        // These are finite stop limits for an always-OFF experiment, not
        // measured or qualified cost bounds. Use downward integer rounding.
        if hz % 2000 != 0
            || hz < 1_000_000
            || (missing == 0 && !ok)
            || (freq != 0 && freq != hz)
            || (hz / 400)
                .checked_mul(super::LAST + 5)
                .and_then(|h| now().checked_add(h))
                .is_none()
            || !within(sum(&[ready, wake, poll, commit, finish]), SENDER_US)
            || !within(sum(&[receiver, control]), 250)
            || !within(sum(&[irq, mask, supervisor]), 500)
        {
            return false;
        }
        let us = |n: u64| (u128::from(hz) * u128::from(n) / 1_000_000) as u64;
        let (ready, wake, poll, commit, finish, mask) =
            (us(250), us(25), us(50), us(15), us(50), us(25));
        unsafe {
            LIMITS = Some(Margins {
                ready,
                wake,
                poll,
                commit,
                finish,
                mask,
            })
        }
        return true;
    }
    if ok {
        unsafe {
            LIMITS = Some(Margins {
                ready,
                wake,
                poll,
                commit,
                finish,
                mask,
            })
        }
    }
    ok
}

pub(crate) fn invalidate() {
    let seq = COMMITTED.load(Ordering::Acquire);
    log(92, now(), unsafe { CONSUMED }, seq);
    unsafe { CONSUMED = seq };
}
fn limits() -> Margins {
    unsafe { LIMITS.unwrap_or_else(|| fail()) }
}
pub(super) fn stopped() -> bool {
    unsafe { HALTED }
}
pub(crate) fn timing_stop(why: u64) {
    if !stopped() {
        unsafe { HALTED = true };
        log(66, now(), 100 + why, unsafe { ATTEMPT });
    }
}
pub(super) fn end_witness() {
    let n = unsafe { (&raw const jnox_permit_returns.count).read_volatile() } as usize;
    for i in 0..n.min(128) {
        let [sp, pc, t] = unsafe {
            (&raw const jnox_permit_returns.rows)
                .cast::<[u64; 3]>()
                .add(i)
                .read_volatile()
        };
        log(93, t, sp, pc);
    }
    let (ctl, cval, daif): (u64, u64, u64);
    unsafe {
        asm!("mrs {},cntp_ctl_el0","mrs {},cntp_cval_el0","mrs {},daif",out(reg)ctl,out(reg)cval,out(reg)daif,options(nostack))
    };
    let pending = super::gic_read(super::GICD + 0x200);
    log(85, now(), cval, ctl);
    log(86, now(), u64::from(pending), daif);
    #[cfg(jnox_permit_off)]
    log(89, now(), app::witness_count(), app::read_output() as u64);
    log(
        90,
        now(),
        unsafe { ATTEMPT },
        7u64.saturating_sub(unsafe { ATTEMPT }),
    );
}
pub(super) fn terminal(s: &Runtime) -> ! {
    for id in 0..2 {
        if s.jobs[id].is_some() {
            log(95, now(), id as u64, s.service[id]);
        }
    }
    end_witness();
    log(18, now(), app::read_output() as u64, 1);
    super::dump()
}
fn reject(why: u64, seq: u64) {
    if stopped() {
        return;
    }
    unsafe { HALTED = true };
    log(66, now(), why, seq);
    app::inhibit(10);
}

pub(super) fn program(s: &Runtime) {
    let control = s.target(s.next);
    if let Some(wake) = unsafe { WAKE }
        && wake < control
    {
        unsafe {
            asm!("msr cntp_cval_el0,{}","msr cntp_ctl_el0,{}","isb",in(reg)wake,in(reg)1u64,options(nostack))
        };
        let (readback, ctl): (u64, u64);
        unsafe {
            asm!("mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0",out(reg)readback,out(reg)ctl,options(nostack))
        };
        log(67, now(), wake, control);
        log(96, now(), readback, ctl);
        if readback != wake || ctl & 3 != 1 {
            reject(16, unsafe { ATTEMPT })
        }
    } else {
        super::arm(control, s.next)
    }
}

pub(super) fn service_wake(s: &mut Runtime, entry: u64) -> bool {
    let Some(target) = (unsafe { WAKE }) else {
        return false;
    };
    if entry < target {
        return false;
    }
    unsafe {
        WAKE = None;
        LAST_WAKE = Some(target)
    };
    if s.jobs[1].is_none() || s.frames[1].is_null() {
        fail()
    }
    s.enqueue(1);
    log(68, entry, target, s.frames[1].addr() as u64);
    true
}

pub(super) fn sleep(s: &mut Runtime, entry: u64) {
    let wake = unsafe {
        let t = REQUEST.unwrap_or_else(|| fail());
        REQUEST = None;
        WAKE = Some(t);
        t
    };
    log(69, entry, wake, s.service[1]);
    if entry >= wake || now() >= wake {
        reject(15, unsafe { ATTEMPT });
        return;
    }
    // If the control PPI became pending during SVC, preserve its existing
    // compare/level. It will run immediately on exception return.
    if now() < s.target(s.next) {
        program(s)
    }
}

fn service_left(s: &Runtime, t: u64) -> Option<u64> {
    let charged = s.service[1].checked_add(t.checked_sub(s.slice_start)?)?;
    let cap = (u128::from(s.frequency) * u128::from(SENDER_US) / 1_000_000) as u64;
    cap.checked_sub(charged)
}

fn attempt() -> Option<(u64, u64, u64)> {
    let s = state();
    let job = s.jobs[1]?;
    let tick = now();
    unsafe { LAST_WAKE = None };
    let seq = job
        .release
        .checked_sub(s.epoch)?
        .checked_div(s.step.checked_mul(4)?)?
        .checked_add(1)?;
    let (_, generation, fault) = app::input_state();
    if unsafe { HALTED } || fault {
        log(70, tick, seq, 0);
        return None;
    }
    if !app::input_state().0 || generation != 1 || unsafe { ATTEMPT }.checked_add(1) != Some(seq) {
        reject(1, seq);
        return None;
    }
    unsafe { ATTEMPT = seq };
    let period = s.step.checked_mul(4)?;
    let nominal = job.release.checked_add(s.step / 5)?;
    let target = if cfg!(jnox_permit_anchor) && seq > 1 {
        unsafe { ANCHOR }?.checked_add((seq - 1).checked_mul(period)?)?
    } else {
        nominal
    };
    let deadline = job.release.checked_add(job.relative_deadline)?;
    log(71, tick, seq, target);
    let m = limits();
    if target < job.release
        || target >= deadline
        || tick > target
        || job.release.checked_add(m.ready).is_none_or(|t| t > target)
        || target.checked_add(m.commit)?.checked_add(m.finish)? > deadline
        || service_left(s, tick)? < m.poll.checked_add(m.commit)?.checked_add(m.finish)?
    {
        reject(2, seq);
        return None;
    }
    Some((seq, target, deadline))
}

pub(super) extern "C" fn sender() -> ! {
    let candidate = locked(|| {
        let result = attempt();
        if result.is_none() && !stopped() {
            reject(12, unsafe { ATTEMPT })
        }
        result
    });
    if let Some((seq, target, deadline)) = candidate {
        let m = limits();
        let wake = target.checked_sub(m.poll).unwrap_or_else(|| fail());
        if now() < wake {
            locked(|| {
                if (unsafe { REQUEST }).is_some() || (unsafe { WAKE }).is_some() {
                    fail()
                }
                unsafe { REQUEST = Some(wake) }
            });
            unsafe { asm!("svc #3", options(nostack)) }
        }
        let usable = locked(|| {
            let s = state();
            let t = now();
            let available = service_left(s, t).unwrap_or(0);
            log(72, t, seq, available);
            unsafe { LAST_WAKE }.is_none_or(|wake| t >= wake && t - wake <= m.wake)
                && !unsafe { HALTED }
                && !s.fault
                && t <= target
                && target - t <= m.poll
                && target - t <= available.saturating_sub(m.commit.saturating_add(m.finish))
                && target
                    .checked_add(m.commit)
                    .and_then(|v| v.checked_add(m.finish))
                    .is_some_and(|end| end <= deadline)
        });
        if usable {
            // IRQ-enabled bounded final poll; an interruption that misses
            // target yields a single missed attempt and never another store.
            let sampled = super::sample_at(target.saturating_sub(m.mask.saturating_sub(m.commit)));
            locked(|| publish(seq, target, sampled, deadline));
        } else {
            locked(|| reject(3, seq))
        }
    }
    unsafe { asm!("svc #2", options(noreturn)) }
}

fn publish(seq: u64, target: u64, polled: u64, deadline: u64) {
    let s = state();
    let m = limits();
    let prep = now();
    let (gate, generation, fault) = app::input_state();
    if !gate
        || fault
        || unsafe { HALTED }
        || s.fault
        || COMMITTED.load(Ordering::Acquire) != seq - 1
        || unsafe { CONSUMED } != seq - 1
        || prep >= deadline
        || service_left(s, prep).unwrap_or(0) < m.commit.saturating_add(m.finish)
    {
        reject(4, seq);
        return;
    }
    log(73, polled, seq, target);
    // The first publish establishes its OWN actual singleton tick, possibly
    // after its nominal earliest target. Later samples are classified against
    // their prescribed nominal/anchored targets with no rounding.
    if (seq > 1 && prep > target) || target.saturating_sub(prep) > m.mask.saturating_sub(m.commit) {
        reject(13, seq);
        return;
    }
    let private = Payload {
        generation,
        seq,
        issued: 0,
    };
    unsafe { (&raw mut SLOT).write(private) };
    let stamp = unsafe { &raw mut SLOT.issued };
    let (i, u): (u64, u64);
    unsafe {
        asm!("2:","isb","mrs {i},cntpct_el0","cmp {i},{target}","b.lo 2b","str {i},[{stamp}]","stlr {seq},[{slot}]","dsb sy","isb","mrs {u},cntpct_el0",
        i=out(reg)i,u=out(reg)u,seq=in(reg)seq,target=in(reg)target,stamp=in(reg)stamp,slot=in(reg)COMMITTED.as_ptr(),options(nostack))
    };
    log(91, prep, u, seq);
    let previous = unsafe { (LAST_I, LAST_U) };
    unsafe {
        LAST_I = i;
        LAST_U = u
    };
    log(11, i, seq, generation);
    log(74, i, u, seq);
    if u < i || i < polled || u >= deadline || u.saturating_sub(prep) > m.mask || u - i > m.commit {
        reject(5, seq);
        return;
    }
    if seq == 1 && i == u {
        unsafe { ANCHOR = Some(i) };
        log(75, i, seq, generation)
    }
    if i != u {
        reject(6, seq);
        return;
    }
    if seq > 1
        && (i != target
            || i.checked_sub(previous.1) != Some(4 * s.step)
            || u.checked_sub(previous.0) != Some(4 * s.step))
    {
        reject(7, seq);
    }
}

pub(super) fn supervisor(target: u64, step: u64) {
    if unsafe { COMMAND_USED || HALTED } {
        return;
    }
    let (gate, generation, fault) = app::input_state();
    if !gate
        || generation != 1
        || fault
        || unsafe { CONSUMED } != 2
        || unsafe { ANCHOR }.and_then(|a| a.checked_add(6 * step)) != Some(target)
    {
        reject(8, 2);
        return;
    }
    unsafe { COMMAND_USED = true };
    let poll = super::sample_at(target);
    let (i, u): (u64, u64);
    unsafe {
        asm!("isb","mrs {i},cntpct_el0","stlr {generation},[{cmd}]","dsb sy","isb","mrs {u},cntpct_el0",
        i=out(reg)i,u=out(reg)u,generation=in(reg)generation,cmd=in(reg)COMMAND.as_ptr(),options(nostack))
    };
    unsafe {
        COMMAND_I = i;
        COMMAND_U = u
    };
    log(76, i, u, target);
    log(77, poll, generation, 0);
    if i != target || u != target {
        reject(9, 2)
    }
}

pub(crate) fn receive(step: u64) {
    let lo = now();
    let seq = COMMITTED.load(Ordering::Acquire);
    if seq != unsafe { CONSUMED } {
        let value = unsafe { (&raw const SLOT).read_volatile() };
        let hi = now();
        log(78, lo, hi, seq);
        if seq != value.seq
            || unsafe { CONSUMED }.checked_add(1) != Some(seq)
            || value.generation != 1
            || value.issued > lo
            || lo - value.issued >= 8 * step
        {
            reject(10, seq);
            return;
        }
        unsafe { CONSUMED = seq };
        if !app::receive_publication(value.generation, value.seq, value.issued, step) {
            reject(14, seq);
            return;
        }
    }
    let lo = now();
    let token = COMMAND.load(Ordering::Acquire);
    let hi = now();
    if token != 0 && !unsafe { COMMAND_SEEN } {
        unsafe { COMMAND_SEEN = true };
        log(79, lo, hi, token);
        let (i, u) = unsafe { (COMMAND_I, COMMAND_U) };
        if token != 1 || i != u || u > lo || unsafe { HALTED } {
            reject(11, token);
            return;
        }
        app::command(i);
    }
}
