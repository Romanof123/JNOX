#[path = "../kernel/src/domain_policy.rs"]
mod domain_policy;
#[path = "../kernel/src/smp_policy.rs"]
mod smp_policy;
#[path = "../kernel/src/pi4_handoff.rs"]
mod pi4_handoff;

use pi4_handoff::*;
use smp_policy::Lease;

const M: Lease = Lease { cpu: 0, task: 0, sequence: 11 };
const P1: Lease = Lease { cpu: 1, task: 2, sequence: 12 };

fn opening_rx() -> Receive {
    Receive { cpu: 0, epoch: 0, iar: 1025, live: M.sequence, claim: Some(M), trap_valid: true }
}

fn opening_state() -> Handoff {
    let mut state = Handoff::new();
    for task in 0..3 {
        state.install(task, 0x1000 + usize::from(task) * 0x1000, Origin::Fresh).unwrap();
    }
    state.publish_opening(0, P1, M).unwrap();
    state
}

fn reverse_state(global: bool) -> (Handoff, Receive) {
    let mut state = opening_state();
    state.consume_opening(opening_rx(), Some(P1), None, true).unwrap();
    state.install(0, 0x1010, Origin::IrqSaved).unwrap();
    state.install(2, 0x3000, Origin::Fresh).unwrap();
    let target = Lease { cpu: 1, task: if global { 0 } else { 2 }, sequence: 14 };
    state.publish_reverse(1, target, global).unwrap();
    (state, Receive { cpu: 1, epoch: 1, iar: 1, live: 0, claim: Some(target), trap_valid: true })
}

fn rejected<T>(state: &mut Handoff, action: impl FnOnce(&mut Handoff) -> Option<T>) {
    let before = *state;
    assert!(action(state).is_none());
    assert_eq!(*state, before);
}

#[test]
fn both_destinations_keep_the_published_identity() {
    for global in [false, true] {
        let (mut state, rx) = reverse_state(global);
        let Slot::Pending(expected) = state.reverse else { panic!() };
        assert_eq!(expected.frame.version, 2);
        assert_eq!(expected.frame.pointer, if global { 0x1010 } else { 0x3000 });
        assert_eq!(state.consume_reverse(rx, global), Some(expected));
        assert_eq!(state.reverse, Slot::Consumed(expected));
        rejected(&mut state, |s| s.consume_reverse(rx, global));
        rejected(&mut state, |s| s.publish_reverse(1, expected.target, global));
    }
}

#[test]
fn opening_requires_retirement_and_actual_local_execution() {
    let mut state = opening_state();
    let rx = opening_rx();
    for bad in [
        Receive { epoch: 1, ..rx }, Receive { cpu: 1, ..rx },
        Receive { iar: 1, ..rx }, Receive { iar: 1026, ..rx },
        Receive { live: 0, ..rx }, Receive { claim: None, ..rx },
        Receive { claim: Some(Lease { sequence: 13, ..M }), ..rx },
        Receive { trap_valid: false, ..rx },
    ] {
        rejected(&mut state, |s| s.consume_opening(bad, Some(P1), None, true));
    }
    rejected(&mut state, |s| s.consume_opening(rx, Some(M), None, true));
    rejected(&mut state, |s| s.consume_opening(rx, Some(P1), Some(P1), true));
    rejected(&mut state, |s| s.consume_opening(rx, Some(P1), None, false));
    rejected(&mut state, |s| s.publish_opening(0, P1, M));
    state.consume_opening(rx, Some(P1), None, true).unwrap();
    rejected(&mut state, |s| s.consume_opening(rx, Some(P1), None, true));
}

#[test]
fn reverse_requires_exact_installation_and_idle_recipient() {
    let (mut state, rx) = reverse_state(true);
    for bad in [
        Receive { iar: 1025, ..rx }, Receive { epoch: 0, ..rx },
        Receive { cpu: 0, ..rx }, Receive { live: 14, ..rx },
        Receive { claim: Some(P1), ..rx }, Receive { trap_valid: false, ..rx },
    ] {
        rejected(&mut state, |s| s.consume_reverse(bad, true));
    }
    rejected(&mut state, |s| s.consume_reverse(rx, false));
    let installed = state.frames[0].unwrap();
    for wrong in [
        FrameInstall { pointer: 0x4010, ..installed },
        FrameInstall { origin: Origin::Fresh, ..installed },
    ] {
        state.frames[0] = Some(wrong);
        rejected(&mut state, |s| s.consume_reverse(rx, true));
    }
    state.frames[0] = Some(installed);
    let newer = state.install(0, installed.pointer, installed.origin).unwrap();
    assert_eq!(newer.version, installed.version + 1);
    rejected(&mut state, |s| s.consume_reverse(rx, true));
}

#[test]
fn empty_slots_bad_publication_and_version_exhaustion() {
    let mut state = Handoff::new();
    rejected(&mut state, |s| s.consume_opening(opening_rx(), Some(P1), None, true));
    rejected(&mut state, |s| s.consume_reverse(opening_rx(), true));
    rejected(&mut state, |s| s.publish_opening(1, P1, M));
    rejected(&mut state, |s| s.publish_opening(0, M, P1));
    rejected(&mut state, |s| s.publish_opening(0, P1, Lease { sequence: 0, ..M }));
    rejected(&mut state, |s| s.install(2, 0x3008, Origin::Fresh));
    state.frames[2] = Some(FrameInstall { pointer: 0x3000, version: u64::MAX, origin: Origin::Fresh });
    rejected(&mut state, |s| s.install(2, 0x4000, Origin::Fresh));
}
