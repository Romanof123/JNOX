//! Single-consumer transaction for the native evaluator. Opening, publication
//! and consumption are serialized by the caller on the masked kernel path.
use super::extension_policy::{Decision, Extension, Op};
use super::group_policy::DelegatedTask;

pub(super) struct Evaluation {
    policy: Extension<3>,
    snapshot: [DelegatedTask; 3],
    cell: u64,
    progress: u64,
}

impl Evaluation {
    pub(super) fn evaluate(mut self, program: &[Op]) -> Reply {
        let decision = self.policy.decide(self.cell, self.progress, &self.snapshot, 1, 1, program, 5);
        Reply { policy: self.policy, decision, cell: self.cell, progress: self.progress, stamp: 0 }
    }
}

pub(super) struct Reply {
    policy: Extension<3>,
    decision: Decision,
    cell: u64,
    progress: u64,
    stamp: u64,
}

pub(super) struct Transaction {
    policy: Extension<3>,
    pub(super) cell: Option<u64>,
    pub(super) cutoff: u64,
    pub(super) snapshot: [DelegatedTask; 3],
    pub(super) heartbeat: u64,
    pub(super) evaluating: bool,
    reply: Option<Reply>,
}

impl Transaction {
    pub(super) fn new(policy: Extension<3>, snapshot: [DelegatedTask; 3]) -> Self {
        Self { policy, cell: None, cutoff: 0, snapshot, heartbeat: 0, evaluating: false, reply: None }
    }

    pub(super) fn abandon(&mut self) {
        self.reply = None;
        self.evaluating = false;
    }

    // The native deployment reserves cell 2 of each three for task 0.
    pub(super) fn open(&mut self, cell: u64, cutoff: u64, snapshot: [DelegatedTask; 3]) -> Option<Decision> {
        self.abandon();
        self.cell = Some(cell);
        self.cutoff = cutoff;
        self.snapshot = snapshot;
        self.evaluating = cell % 3 != 2;
        if self.evaluating { None } else {
            Some(self.policy.decide(cell, self.heartbeat, &self.snapshot, 1, 1, &[], 5))
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.evaluating && self.reply.is_none()
    }

    pub(super) fn begin(&self) -> Option<Evaluation> {
        if !self.pending() { return None; }
        Some(Evaluation { policy: self.policy, snapshot: self.snapshot, cell: self.cell?,
                          progress: self.heartbeat.checked_add(1)? })
    }

    pub(super) fn publish(&mut self, reply: Reply, counter: impl FnOnce() -> u64) -> bool {
        if !self.pending() || self.cell != Some(reply.cell) { return false; }
        self.reply = Some(reply);
        // The counter must be sampled after staging, before IRQs are unmasked.
        let stamp = counter();
        if stamp >= self.cutoff {
            self.reply = None;
            return false;
        }
        self.reply.as_mut().unwrap().stamp = stamp;
        true
    }

    pub(super) fn consume(&mut self, cell: u64, now: u64) -> Option<(Decision, Option<u64>)> {
        if !self.evaluating || self.cell != Some(cell) || now < self.cutoff { return None; }
        self.evaluating = false;
        if let Some(reply) = self.reply.take() {
            self.policy = reply.policy;
            self.heartbeat = reply.progress;
            Some((reply.decision, Some(reply.stamp)))
        } else {
            Some((self.policy.decide(cell, self.heartbeat, &self.snapshot, 1, 1, &[], 5), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::extension_policy::{Reason, Reservation};
    use super::super::group_policy::Proposal;

    const Q: u64 = 6_250_000;
    const REQUEST: Proposal = Proposal { task: 2, generation: 17, cpu: 0, ticks: 1 };
    const PROGRAM: [Op; 1] = [Op::Propose(REQUEST)];

    fn tasks(mask: u8) -> [DelegatedTask; 3] {
        core::array::from_fn(|i| DelegatedTask {
            generation: [7, 11, 17][i], ready: mask & (1 << i) != 0,
            allowed_cpus: 1, grant_limit: 1,
        })
    }

    fn new() -> Transaction {
        let reserve = Reservation { task: 0, period: 3, offset: 2, ticks: 1 };
        Transaction::new(Extension::new(reserve, 0, 0, 2).unwrap(), tasks(0))
    }

    fn cutoff(k: u64) -> u64 { k * Q + Q / 4 }

    fn open(tx: &mut Transaction, k: u64, mask: u8) -> Option<Decision> {
        let result = tx.open(k, cutoff(k), tasks(mask));
        println!("open cell={k} mask={mask} cutoff={} result={result:?} heartbeat={}", cutoff(k), tx.heartbeat);
        result
    }

    fn observe(tx: &mut Transaction, k: u64) -> (Decision, Option<u64>) {
        let stored = tx.reply.as_ref().map(|r| (r.cell, r.stamp));
        let result = tx.consume(k, cutoff(k)).unwrap();
        println!("consume cell={k} at={} stored={stored:?} result={result:?} heartbeat={}",
                 cutoff(k), tx.heartbeat);
        result
    }

    fn confirm(tx: &mut Transaction, k: u64) {
        assert!(open(tx, k, 7).is_none());
        let reply = tx.begin().unwrap().evaluate(&PROGRAM);
        let old = tx.heartbeat;
        assert!(tx.publish(reply, || cutoff(k)-1));
        assert_eq!(tx.heartbeat, old);
        assert_eq!(tx.reply.as_ref().map(|r| (r.cell,r.stamp)), Some((k,cutoff(k)-1)));
        assert!(tx.consume(k, cutoff(k)-1).is_none());
        let (decision, stamp) = observe(tx, k);
        assert_eq!(stamp, Some(cutoff(k)-1));
        assert_eq!((decision.reason, decision.steps, decision.selected), (Reason::Accepted,1,Some(REQUEST)));
        assert_eq!(tx.heartbeat, old+1);
        assert!(tx.consume(k, cutoff(k)).is_none());
    }

    #[test]
    fn publication_and_absence_at_cutoff() {
        confirm(&mut new(), 0);
        for late in [Some(0), Some(1), None] {
            for (mask, expected) in [(7,Some(1)),(1,None)] {
                let mut tx = new();
                open(&mut tx, 0, mask);
                if let Some(delay) = late {
                    let reply = tx.begin().unwrap().evaluate(&PROGRAM);
                    let at = cutoff(0) + delay;
                    println!("publication cell=0 stamp={at} cutoff={}", cutoff(0));
                    assert!(!tx.publish(reply, || at));
                }
                let (d, stamp) = observe(&mut tx, 0);
                assert_eq!((stamp, tx.heartbeat, d.reason, d.steps), (None,0,Reason::Absent,0));
                assert_eq!(d.selected.map(|p| (p.task,p.ticks)), expected.map(|id| (id,1)));
                assert!(!tx.evaluating && tx.reply.is_none());
            }
        }
    }

    #[test]
    fn obsolete_producer_cannot_change_a_newer_transaction() {
        for newer_published in [false,true] {
            let mut tx = new();
            open(&mut tx, 0, 7);
            let obsolete = tx.begin().unwrap().evaluate(&PROGRAM);
            open(&mut tx, 1, 7);
            if newer_published {
                let fresh = tx.begin().unwrap().evaluate(&PROGRAM);
                assert!(tx.publish(fresh, || cutoff(1)-1));
            }
            assert!(!tx.publish(obsolete, || cutoff(0)-1));
            assert!(tx.consume(0, cutoff(1)).is_none());
            assert_eq!((tx.cell, tx.heartbeat), (Some(1),0));
            assert_eq!(tx.reply.as_ref().map(|p| p.cell), newer_published.then_some(1));
            let (d, stamp) = observe(&mut tx, 1);
            assert_eq!(stamp.is_some(), newer_published);
            assert_eq!(tx.heartbeat, u64::from(newer_published));
            assert_eq!(d.reason, if newer_published {Reason::Accepted} else {Reason::Absent});
            assert!(tx.consume(1, cutoff(1)).is_none());
        }
        let mut tx = new();
        open(&mut tx, 0, 7);
        let late = tx.begin().unwrap().evaluate(&PROGRAM);
        let staged = tx.begin().unwrap().evaluate(&PROGRAM);
        assert!(tx.publish(staged, || cutoff(0)-1));
        tx.abandon();
        assert!(tx.reply.is_none());
        assert!(!tx.publish(late, || cutoff(0)-1));
        assert!(tx.begin().is_none() && tx.consume(0, cutoff(0)).is_none());
        assert_eq!((tx.cell,tx.heartbeat),(Some(0),0));
    }

    #[test]
    fn missing_progress_reservations_and_inclusive_watchdog() {
        for confirmed in [0,1] {
            for (mask, selected) in [(7,Some(0)),(6,None)] {
                let mut tx = new();
                confirm(&mut tx, confirmed);
                if confirmed == 0 {
                    open(&mut tx, 1, 7);
                    let (d,stamp) = observe(&mut tx,1);
                    assert_eq!((d.reason,d.selected.unwrap().task,stamp), (Reason::Absent,1,None));
                }
                let reserved = open(&mut tx,2,mask).unwrap();
                assert_eq!((reserved.reason,reserved.steps,reserved.selected.map(|p| p.task)),
                           (Reason::Reserved,0,selected));
                assert!(tx.begin().is_none() && tx.consume(2,cutoff(2)).is_none());
                assert_eq!(tx.heartbeat,1);
                open(&mut tx,3,7);
                let (d,stamp) = observe(&mut tx,3);
                assert_eq!((d.reason,d.selected.unwrap().task,stamp,tx.heartbeat), (Reason::Stalled,1,None,1));
                confirm(&mut tx,4);
                open(&mut tx,5,7);
                open(&mut tx,6,7);
                let regressed = tx.policy.decide(6,1,&tx.snapshot,1,1,&PROGRAM,5);
                assert_eq!((regressed.reason, tx.heartbeat),(Reason::InvalidObservation,2));
                let repeated = tx.policy.decide(6,1,&tx.snapshot,1,1,&PROGRAM,5);
                assert_eq!(repeated.reason,Reason::InvalidObservation);
                assert_eq!(observe(&mut tx,6).0.reason,Reason::Stalled);
            }
        }
    }
}
