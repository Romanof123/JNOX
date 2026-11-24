//! Discrete reference for the three-task MLQ/MLFQ profiles. The oracle keeps
//! lifetime service since the last boost and a literal queue of ready IDs.

use crate::multilevel_policy::{Multilevel, Stop};
use std::collections::VecDeque;

const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0],
];
const ALLOT: [u64; 3] = [3, 2, 4];
const QUANTUM: u64 = 3;

struct Reference {
    classes: Option<[u8; 3]>,
    ready: VecDeque<usize>,
    running: Option<usize>,
    service: [u64; 3],
}

impl Reference {
    fn rank_and_grant(&self, id: usize) -> (u8, u64) {
        if let Some(classes) = self.classes {
            return (classes[id], QUANTUM);
        }
        let used = self.service[id];
        let [top, middle, bottom] = ALLOT;
        if used < top {
            (2, top - used)
        } else if used < top + middle {
            (1, top + middle - used)
        } else {
            (0, bottom - (used - top - middle) % bottom)
        }
    }

    fn wake(&mut self, id: usize) -> bool {
        if id >= 3 || self.running == Some(id) || self.ready.contains(&id) {
            return false;
        }
        self.ready.push_back(id);
        true
    }

    fn dispatch(&mut self) -> Option<usize> {
        if self.running.is_some() {
            return None;
        }
        let rank = self.ready.iter().map(|&id| self.rank_and_grant(id).0).max()?;
        let first = self.ready.iter().position(|&id| self.rank_and_grant(id).0 == rank)?;
        self.running = self.ready.remove(first);
        self.running
    }

    fn stop(&mut self, id: usize, service: u64, why: Stop) -> bool {
        if self.running != Some(id) {
            return false;
        }
        let (_, grant) = self.rank_and_grant(id);
        if service > grant || (why == Stop::Expire && service != grant) {
            return false;
        }
        self.service[id] += service;
        self.running = None;
        if why != Stop::Sleep {
            self.ready.push_back(id);
        }
        true
    }

    fn boost(&mut self) -> bool {
        if self.classes.is_some() || self.running.is_some() {
            return false;
        }
        self.service = [0; 3];
        true
    }
}

struct Trace {
    model: Reference,
    policy: Multilevel<3>,
}

impl Trace {
    fn new(classes: Option<[u8; 3]>) -> Self {
        let policy = match classes {
            Some(fixed) => Multilevel::fixed(fixed, QUANTUM),
            None => Multilevel::feedback(ALLOT),
        }.unwrap();
        Self {
            model: Reference {
                classes,
                ready: VecDeque::new(),
                running: None,
                service: [0; 3],
            },
            policy,
        }
    }

    // Inspect sleepers and queued peers too, so an accidental charge to
    // another task is observable at the same transition.
    fn check(&self) {
        for id in 0..3 {
            let (rank, grant) = self.model.rank_and_grant(id);
            assert_eq!(self.policy.priority(id), Some(rank), "rank of {id}");
            assert_eq!(self.policy.remaining(id), Some(grant), "grant of {id}");
        }
    }

    fn wake(&mut self, id: usize) {
        assert_eq!(self.policy.wake(id), self.model.wake(id), "wake {id}");
        self.check();
    }

    fn select(&mut self) -> Option<usize> {
        let expected = self.model.dispatch();
        assert_eq!(self.policy.dispatch(), expected, "dispatch");
        self.check();
        expected
    }

    fn stop(&mut self, id: usize, service: u64, why: Stop) {
        assert_eq!(
            self.policy.stop(id, service, why),
            self.model.stop(id, service, why),
            "stop {id} {service} {why:?}"
        );
        self.check();
    }

    fn boost(&mut self) {
        assert_eq!(self.policy.boost(), self.model.boost(), "boost");
        self.check();
    }
}

#[test]
fn strict_class_matches_all_arrival_orders_and_low_class_placements() {
    for low in 0..3 {
        let classes = core::array::from_fn(|id| if id == low { 1 } else { 9 });
        for order in ORDERS {
            let mut trace = Trace::new(Some(classes));
            for id in order {
                trace.wake(id);
                trace.wake(id);
            }
            let peers: Vec<_> = order.into_iter().filter(|&id| id != low).collect();
            for turn in 0..12 {
                let id = trace.select().unwrap();
                assert_eq!(id, peers[turn % 2]);
                trace.boost();
                trace.stop(low, 1, Stop::Sleep);
                if turn % 2 == 0 {
                    trace.stop(id, QUANTUM, Stop::Expire);
                } else {
                    trace.stop(id, 1, Stop::Yield);
                }
                trace.stop(id, 1, Stop::Yield);
            }
            for id in peers {
                assert_eq!(trace.select(), Some(id));
                trace.stop(id, 0, Stop::Sleep);
            }
            trace.boost();
            assert_eq!(trace.select(), Some(low));
        }
    }
}

fn feedback_prefix(order: [usize; 3]) -> Trace {
    let [cpu, interactive, other] = order;
    let mut trace = Trace::new(None);
    for id in order {
        trace.wake(id);
    }
    // Two early yields and one expiry consume the CPU task's top allotment.
    // Its peers sleep. The returning interactive peer keeps its unused grant.
    for (id, units, why) in [
        (cpu, 1, Stop::Yield),
        (interactive, 1, Stop::Sleep),
        (other, 0, Stop::Sleep),
        (cpu, 1, Stop::Yield),
        (cpu, 1, Stop::Expire),
    ] {
        assert_eq!(trace.select(), Some(id));
        trace.stop(id, units, why);
    }
    trace.wake(interactive);
    assert_eq!(trace.select(), Some(interactive));
    trace.stop(interactive, 0, Stop::Sleep);
    assert_eq!(trace.select(), Some(cpu));
    trace.stop(cpu, ALLOT[1], Stop::Expire);
    assert_eq!(trace.select(), Some(cpu));
    trace.stop(cpu, 1, Stop::Yield);
    trace.wake(other);
    // CPU is first in the ready queue at bottom rank, followed by a top-rank
    // peer. Boosting now and selecting without a boost yield different winners.
    trace
}

#[test]
fn feedback_matches_all_five_stop_sequences_and_irregular_boost_epochs() {
    const ROUNDS: usize = 5;
    const EPOCHS: [&[usize]; 4] = [&[], &[0, 1, 4], &[0, 3, 4], &[2]];
    for order in ORDERS {
        for epochs in EPOCHS {
            for sequence in 0..3usize.pow(ROUNDS as u32) {
                let mut trace = feedback_prefix(order);
                let mut choices = sequence;
                for tick in 0..ROUNDS {
                    if epochs.contains(&tick) {
                        trace.boost();
                    }
                    if tick == 1 || tick == 3 {
                        trace.wake(order[1]);
                    }
                    let id = trace.select().unwrap_or_else(|| {
                        trace.wake(order[tick % 3]);
                        trace.select().unwrap()
                    });
                    if tick == 0 {
                        assert_eq!(id, if epochs.contains(&0) { order[0] } else { order[2] });
                    }
                    let (_, grant) = trace.model.rank_and_grant(id);
                    assert_eq!(trace.select(), None);
                    trace.wake(id);
                    trace.wake(usize::MAX);
                    trace.boost();
                    trace.stop(3, 0, Stop::Sleep);
                    trace.stop((id + 1) % 3, 1, Stop::Yield);
                    trace.stop(id, grant - 1, Stop::Expire);
                    trace.stop(id, u64::MAX, Stop::Sleep);
                    match choices % 3 {
                        0 => trace.stop(id, 1, Stop::Yield),
                        1 => trace.stop(id, grant, Stop::Expire),
                        _ => trace.stop(id, grant / 2, Stop::Sleep),
                    }
                    trace.stop(id, 1, Stop::Yield);
                    choices /= 3;
                }
                // Inspect the last stop's eligibility and tie order by draining.
                while let Some(id) = trace.select() {
                    trace.stop(id, 0, Stop::Sleep);
                }
            }
        }
    }
}
