//! Deterministic CPU0 policy boundaries. Counters below are delivered guest ticks.
#[path = "../kernel/src/domain_policy.rs"] mod domain_policy;
#[path = "../kernel/src/ready_queue.rs"] mod ready_queue;
#[path = "../kernel/src/native_priority_threads.rs"] mod native_priority_threads;

use native_priority_threads::{Completion, Policy, State};

const OWNER: usize = 0;
const PEER: usize = 1;
const HIGH: usize = 2;
const BACKGROUND: usize = 3;
const ADDRESS: u64 = 0x1000;
const D: u64 = 150;
const TIMEOUT: i32 = 110;

fn enter(p: &mut Policy, entry: u64, decision: u64) {
    p.pause(entry);
    p.advance(decision);
}

fn returns(p: &mut Policy, expected: usize, stamp: u64) {
    p.dispatch();
    assert_eq!(p.current(), Some(expected));
    assert_eq!(p.state(expected), State::Running);
    p.resume(stamp);
}

fn lock(p: &Policy, token: u64, owner: Option<u64>, waiter: Option<u64>) {
    let m = &p.mutex_slots()[(token & 3) as usize];
    assert_eq!(m.address, Some(ADDRESS));
    assert_eq!(m.generation, token >> 2);
    assert_eq!(m.owner, owner);
    assert_eq!(m.waiters, [waiter, None, None, None]);
}

// OWNER and PEER use RR at priority 2. FIFO HIGH sleeps until tick 130.
// BACKGROUND exists but its priority 1 never wins.
fn contended() -> (Policy, u64) {
    let mut p = Policy::new([2, 2, 4, 1], Some(100));
    p.init_epoch(0);
    p.resume(0);
    enter(&mut p, 0, 0);
    let token = p.mutex_init(ADDRESS, 1, 15, 0).unwrap();
    returns(&mut p, OWNER, 0);
    enter(&mut p, 0, 0);
    p.mutex(15, ADDRESS, token, None).unwrap();
    returns(&mut p, OWNER, 0);
    for (role, priority) in [(BACKGROUND, 1), (PEER, 2), (HIGH, 4)] {
        enter(&mut p, 0, 0);
        let id = p.create(Some((role == PEER, priority))).unwrap();
        assert_eq!(id, p.handle(role));
        returns(&mut p, if role == HIGH { HIGH } else { OWNER }, 0);
    }
    assert!(!p.is_rr(HIGH));
    assert!(p.is_rr(OWNER) && p.is_rr(PEER));
    enter(&mut p, 0, 0);
    p.sleep(130);
    returns(&mut p, OWNER, 100);
    assert_eq!(p.next_timer(100), Some(130));
    enter(&mut p, 130, 130);
    assert_eq!(p.residual(OWNER), 70);
    assert_eq!(p.effective(OWNER), 2);
    assert_eq!(p.state(HIGH), State::Ready);
    returns(&mut p, HIGH, 130);
    assert_eq!(p.next_timer(130), None);
    enter(&mut p, 130, 130);
    p.mutex(15, ADDRESS, token, Some(D)).unwrap();
    lock(&p, token, Some(p.handle(OWNER)), Some(p.handle(HIGH)));
    assert_eq!(p.state(HIGH), State::MutexWait(token, Some(D)));
    assert_eq!(p.effective(OWNER), 4);
    assert_eq!(p.effective(HIGH), 4);
    assert_eq!(p.effective(PEER), 2);
    assert_eq!(p.residual(OWNER), 70);
    returns(&mut p, OWNER, 130);
    assert_eq!(p.next_timer(130), Some(D));
    (p, token)
}

#[test]
fn donation_and_withdrawal_preserve_one_rr_turn() {
    let (mut p, token) = contended();
    enter(&mut p, D, D);
    assert_eq!(p.current(), Some(OWNER));
    assert_eq!(p.effective(OWNER), 2);
    assert_eq!(p.residual(OWNER), 50);
    for id in [HIGH, PEER, BACKGROUND] { assert_eq!(p.state(id), State::Ready); }
    lock(&p, token, Some(p.handle(OWNER)), None);
    returns(&mut p, HIGH, D);
    assert_eq!(p.take_completion(HIGH), Some(Completion::Mutex(TIMEOUT)));
    assert_eq!(p.take_completion(HIGH), None);
    assert_eq!(p.next_timer(D), None);
    // FIFO runs for more than Q. None of this time belongs to OWNER.
    enter(&mut p, 260, 261);
    returns(&mut p, HIGH, 262);
    assert_eq!(p.residual(OWNER), 50);
    enter(&mut p, 264, 265);
    p.thread_exit(77);
    returns(&mut p, OWNER, 270);
    assert_eq!(p.next_timer(270), Some(320));
    enter(&mut p, 319, 319);
    assert_eq!(p.residual(OWNER), 1);
    returns(&mut p, OWNER, 319);
    assert_eq!(p.next_timer(319), Some(320));
    enter(&mut p, 320, 320);
    assert_eq!(p.residual(OWNER), 0);
    returns(&mut p, PEER, 320);
    assert_eq!(p.residual(OWNER), 100);
    assert_eq!(p.state(OWNER), State::Ready);
    assert_eq!(p.state(HIGH), State::Zombie(77));
    lock(&p, token, Some(p.handle(OWNER)), None);
}

fn unlock_boundary(at: u64) {
    let (mut p, token) = contended();
    let owner = p.handle(OWNER);
    let high = p.handle(HIGH);
    let early = at < D;
    enter(&mut p, at, at);
    assert_eq!(p.current(), Some(OWNER));
    assert_eq!(p.state(PEER), State::Ready);
    if early {
        lock(&p, token, Some(owner), Some(high));
        assert_eq!(p.effective(OWNER), 4);
        assert_eq!(p.state(HIGH), State::MutexWait(token, Some(D)));
        assert_eq!(p.take_completion(HIGH), None);
    } else {
        lock(&p, token, Some(owner), None);
        assert_eq!(p.effective(OWNER), 2);
        assert_eq!(p.state(HIGH), State::Ready);
        assert_eq!(p.ready_order().last(), Some(&HIGH));
    }
    p.mutex(17, ADDRESS, token, None).unwrap();
    assert_eq!(p.effective(OWNER), 2);
    assert_eq!(p.effective(HIGH), 4);
    assert_eq!(p.state(HIGH), State::Ready);
    assert_eq!(p.ready_order().last(), Some(&HIGH));
    lock(&p, token, early.then_some(high), None);
    returns(&mut p, HIGH, at);
    assert_eq!(p.next_timer(at), None);
    let later = at.max(D);
    enter(&mut p, later, later);
    lock(&p, token, early.then_some(high), None);
    assert_eq!(p.take_completion(HIGH), Some(Completion::Mutex(if early { 0 } else { TIMEOUT })));
    assert_eq!(p.take_completion(HIGH), None);
    if early { p.mutex(17, ADDRESS, token, None).unwrap(); }
    returns(&mut p, HIGH, later);
    enter(&mut p, later, later);
    p.mutex(14, ADDRESS, token, None).unwrap();
    returns(&mut p, HIGH, later);
    enter(&mut p, later, later);
    let fresh = p.mutex_init(ADDRESS, 1, 15, 0).unwrap();
    assert_eq!(fresh & 3, token & 3);
    assert_eq!(fresh >> 2, (token >> 2) + 1);
    returns(&mut p, HIGH, later);
    enter(&mut p, later, later);
    p.mutex(15, ADDRESS, fresh, None).unwrap();
    returns(&mut p, HIGH, later);
    enter(&mut p, later + 1, later + 1);
    assert_eq!(p.mutex(17, ADDRESS, token, None), Err(22));
    lock(&p, fresh, Some(high), None);
    assert_eq!(p.handle(OWNER), owner);
    assert_eq!(p.handle(HIGH), high);
    assert_eq!(p.take_completion(HIGH), None);
    assert_eq!(p.next_timer(later + 1), None);
}

#[test] fn unlock_before_deadline() { unlock_boundary(D - 1); }
#[test] fn unlock_at_deadline() { unlock_boundary(D); }
#[test] fn unlock_after_deadline() { unlock_boundary(D + 1); }
