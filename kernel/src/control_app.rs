use super::control_runtime::{log, now, sample_at};
use core::ptr;

// QEMU virt PL061, masked GPIODATA for bit zero and GPIODIR.
const DATA: *mut u32 = 0x0903_0004 as *mut u32;
const DIR: *mut u32 = 0x0903_0400 as *mut u32;

pub(super) fn read_output() -> bool {
    unsafe { ptr::read_volatile(DATA) & 1 != 0 }
}
pub(super) fn write_output(enabled: bool) {
    unsafe {
        ptr::write_volatile(DATA, enabled as u32);
        core::arch::asm!("dsb sy", options(nostack))
    };
}

#[derive(Clone, Copy)]
struct Permit {
    generation: u64,
    seq: u64,
    issued: u64,
    set: bool,
}
struct App {
    previous: Option<Permit>,
    last: Option<Permit>,
    queued: Option<Permit>,
    generation: u64,
    faulted: bool,
    can_rearm: bool,
    enabled: bool,
    gate: bool,
    observed: bool,
    command: Option<u64>,
    first_issue: Option<u64>,
    expired_before_delivery: Option<u64>,
    previous_check: Option<u64>,
}
static mut APP: App = App {
    previous: None,
    last: None,
    queued: None,
    generation: 1,
    faulted: false,
    can_rearm: true,
    enabled: false,
    gate: true,
    observed: false,
    command: None,
    first_issue: None,
    expired_before_delivery: None,
    previous_check: None,
};

fn app() -> &'static mut App {
    unsafe { &mut *(&raw mut APP) }
}
pub(super) fn init() {
    write_output(false);
    unsafe { ptr::write_volatile(DIR, ptr::read_volatile(DIR) | 1) };
    log(19, now(), read_output() as u64, 0);
}

pub(super) fn observe() {
    let before = now();
    let value = read_output();
    let at = now();
    let a = app();
    if value != a.observed {
        log(27, before, at, value as u64);
        log(14, at, a.observed as u64, value as u64);
        a.observed = value
    }
}

pub(super) fn inhibit(reason: u64) {
    let a = app();
    if !a.faulted {
        let before = now();
        a.faulted = true;
        let after = now();
        log(28, before, after, reason);
        log(
            13,
            after,
            reason,
            a.expired_before_delivery
                .unwrap_or_else(|| a.last.map_or(0, |p| p.issued)),
        );
    }
    a.can_rearm = false;
    a.enabled = false;
    write_output(false);
}

pub(super) fn send(index: u64, step: u64, issued: u64) {
    let a = app();
    if !a.gate {
        log(23, now(), index, 0);
        return;
    }
    let p = Permit {
        generation: a.generation,
        seq: index / 4 + 1,
        issued,
        set: true,
    };
    log(11, issued, p.seq, p.generation);
    if enqueue(p) {
        deliver(step)
    }
    if cfg!(jnox_control_case = "loss") && index == 12 {
        close();
    }
}

fn enqueue(p: Permit) -> bool {
    let a = app();
    if !a.gate {
        return false;
    }
    if let Some(retained) = a.queued {
        log(26, now(), p.seq, retained.seq);
        return false;
    }
    a.queued = Some(p);
    true
}

fn close() {
    let a = app();
    let before = now();
    a.gate = false;
    a.queued = None;
    let after = now();
    log(29, before, after, 0);
    log(
        16,
        after,
        a.last.map_or(0, |p| p.seq),
        a.last.map_or(0, |p| p.issued),
    );
}

fn deliver(step: u64) {
    let a = app();
    let begin = now();
    let incoming = a.queued.take().unwrap();
    let valid = (!a.faulted || a.can_rearm)
        && incoming.generation == a.generation
        && incoming.set
        && incoming.issued <= begin
        && begin - incoming.issued < 8 * step
        && a.last
            .is_none_or(|old| incoming.seq == old.seq + 1 && incoming.issued > old.issued);
    if !valid {
        log(24, begin, incoming.seq, 0);
        return;
    }
    if let Some(old) = a.last
        && begin - old.issued >= 8 * step
    {
        a.expired_before_delivery.get_or_insert(old.issued);
    }
    a.previous = a.last;
    a.last = Some(incoming);
    a.first_issue.get_or_insert(incoming.issued);
    let delivery = now();
    log(30, begin, delivery, incoming.seq);
    log(12, delivery, incoming.seq, incoming.issued);
}

// A one-shot supervisor command, independent of permit sequence progression.
pub(super) fn command_target(step: u64) -> Option<u64> {
    let a = app();
    if a.command.is_none() && a.last.is_some_and(|p| p.seq == 2) && !a.enabled && !a.faulted {
        a.first_issue.map(|first| first + 6 * step)
    } else {
        None
    }
}

pub(super) fn command(issued: u64) {
    app().command = Some(issued);
    log(17, issued, app().generation, 0)
}

pub(super) fn check(index: u64, step: u64) {
    let decision = now();
    let a = app();
    if (a.expired_before_delivery.is_some()
        || a.last.is_some_and(|p| decision - p.issued >= 8 * step))
        && !a.faulted
    {
        inhibit(0);
    } else if let Some(command) = a.command.take() {
        if a.can_rearm
            && let (Some(first), Some(second)) = (a.previous, a.last)
            && second.seq == first.seq + 1
            && second.issued - first.issued == 4 * step
            && command >= second.issued
            && command - first.issued < 8 * step
            && decision - first.issued < 8 * step
        {
            a.faulted = false;
            a.enabled = true;
            write_output(true);
            log(20, now(), first.issued, second.issued);
        }
    }
    let done = now();
    let a = app();
    log(21, done, index, decision);
    if !SCRIPTED
        && a.previous_check
            .is_some_and(|previous| done - previous > 2 * step)
    {
        super::control_runtime::spacing_failure()
    }
    app().previous_check = Some(done);
}

pub(super) const SCRIPTED: bool = cfg!(any(
    jnox_control_case = "worst",
    jnox_control_case = "tie",
    jnox_control_case = "early-command",
    jnox_control_case = "expired-command",
    jnox_control_case = "no-command",
    jnox_control_case = "missing-permit",
    jnox_control_case = "invalid",
    jnox_control_case = "silence",
    jnox_control_case = "saturation",
    jnox_control_case = "recovery",
    jnox_control_case = "old-generation"
));

// Separately selected counter-driven fixture. Every planned target is accompanied
// by its sampled event. A missed equality cannot become an ordinary-workload pass.
pub(super) fn script(hz: u64) {
    let step = hz / 400;
    let origin = now() + step;
    log(25, origin, hz, 0);
    let recovery = cfg!(any(
        jnox_control_case = "recovery",
        jnox_control_case = "old-generation",
        jnox_control_case = "missing-permit",
        jnox_control_case = "early-command",
        jnox_control_case = "no-command",
        jnox_control_case = "expired-command"
    ));
    let mut start = origin;
    if recovery {
        let first = sample_at(origin);
        send(0, step, first);
        send(4, step, sample_at(first + 4 * step));
        command(sample_at(first + 6 * step));
        check(0, step);
        observe();
        close();
        sample_at(first + 12 * step);
        check(1, step);
        observe();
        let a = app();
        if !cfg!(jnox_control_case = "old-generation") {
            a.generation += 1;
            a.can_rearm = true;
            a.previous = None;
            a.last = None;
            a.first_issue = None;
            a.expired_before_delivery = None;
            a.command = None;
        }
        a.gate = true;
        log(31, now(), a.generation, a.can_rearm as u64);
        start = now() + step;
    }
    let first = sample_at(start);
    send(0, step, first);
    if cfg!(jnox_control_case = "early-command") {
        command(sample_at(first + 2 * step));
        check(0, step);
    }
    if !cfg!(jnox_control_case = "missing-permit") {
        send(4, step, sample_at(first + 4 * step));
    }
    if !cfg!(any(
        jnox_control_case = "no-command",
        jnox_control_case = "early-command"
    )) {
        let offset = if cfg!(jnox_control_case = "expired-command") {
            8
        } else {
            6
        };
        command(sample_at(first + offset * step));
    }
    check(1, step);
    observe();
    if cfg!(any(jnox_control_case = "worst", jnox_control_case = "tie")) {
        let issued = sample_at(first + 8 * step);
        send(8, step, issued);
        if cfg!(jnox_control_case = "worst") {
            close()
        }
        // Check intermediate points so the artificial gap never causes a
        // timing-failure latch. The preceding actual completion is also logged.
        for phase in 0..4 {
            sample_at(issued + phase * 2 * step);
            check(2 + phase, step);
            observe();
        }
        let target = issued + 8 * step - u64::from(cfg!(jnox_control_case = "worst"));
        sample_at(target);
        check(6, step);
        if cfg!(jnox_control_case = "tie") {
            send(12, step, now())
        }
        observe();
        if cfg!(jnox_control_case = "worst") {
            sample_at(target + 2 * step);
            check(7, step);
            observe();
        }
    }
    if cfg!(any(
        jnox_control_case = "invalid",
        jnox_control_case = "saturation"
    )) {
        let base = app().last.unwrap();
        if cfg!(jnox_control_case = "invalid") {
            for invalid in [
                Permit {
                    generation: 0,
                    ..base
                },
                base,
                Permit { seq: 0, ..base },
                Permit {
                    seq: 3,
                    issued: now() + step,
                    ..base
                },
                Permit {
                    seq: 3,
                    issued: now().saturating_sub(8 * step),
                    ..base
                },
                Permit {
                    seq: 3,
                    set: false,
                    ..base
                },
            ] {
                if enqueue(invalid) {
                    deliver(step)
                }
            }
        } else {
            enqueue(Permit {
                seq: 3,
                issued: now(),
                ..base
            });
            enqueue(Permit {
                seq: 4,
                issued: now(),
                ..base
            });
            close();
        }
    }
    if cfg!(any(
        jnox_control_case = "invalid",
        jnox_control_case = "silence",
        jnox_control_case = "saturation"
    )) {
        for phase in 7..=13 {
            sample_at(first + phase * step);
            check(phase, step);
            observe();
        }
        let a = app();
        log(
            36,
            now(),
            a.last.map_or(0, |p| p.seq),
            a.last.map_or(0, |p| p.issued),
        );
    }
    log(18, now(), read_output() as u64, 0);
}
