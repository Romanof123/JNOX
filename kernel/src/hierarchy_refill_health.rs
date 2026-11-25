//! Boot-lifetime accounting for one outstanding native hierarchy timer arm.
//! The diagnostic journal can discard detail without resetting this ledger.

pub const VERSION: u64 = 1;

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmKind {
    Other = 0,
    Refill = 1,
    WakeRefill = 2,
    Wake = 3,
    Grant = 4,
    Horizon = 5,
    Terminal = 6,
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Other = 0,
    Scored = 1,
    Late = 2,
    Early = 3,
    Invalid = 4,
    Stale = 5,
    NonPpi = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm {
    pub seq: u64,
    pub kind: ArmKind,
    pub due: u64,
    pub before: u64,
    pub after: u64,
    pub cval: u64,
    pub ctl: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub earliest: u64,
    pub cval: u64,
    pub ctl: u64,
    pub counter: u64,
    pub spsr: u64,
    pub elr: u64,
    pub iar: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    pub outcome: Outcome,
    pub event: u64,
    pub arm: Arm,
    pub entry: Entry,
    pub lag: Option<u64>,
}

pub struct Health {
    pub arms: u64,
    pub events: u64,
    pub scored: u64,
    pub early: u64,
    pub invalid: u64,
    pub late: u64,
    pub max_lag: u64,
    pub overflow: bool,
    pub current: Option<Arm>,
    pub first: Option<Violation>,
}

// Saturate only to preserve evidence. The overflow flag precludes a clean verdict.
fn increment(count: &mut u64, overflow: &mut bool) -> Option<u64> {
    if let Some(next) = count.checked_add(1) {
        *count = next;
        Some(next)
    } else {
        *overflow = true;
        None
    }
}

impl Health {
    pub const fn new() -> Self {
        Self {
            arms: 0, events: 0, scored: 0, early: 0, invalid: 0, late: 0,
            max_lag: 0, overflow: false, current: None, first: None,
        }
    }

    pub fn arm(&mut self, kind: ArmKind, due: u64, before: u64, after: u64,
               cval: u64, ctl: u64) -> u64 {
        let Some(seq) = increment(&mut self.arms, &mut self.overflow) else {
            self.current = None;
            return 0;
        };
        self.current = Some(Arm { seq, kind, due, before, after, cval, ctl });
        seq
    }

    pub fn observe(&mut self, seq: u64, entry: Entry, idle: bool, limit: u64) -> Outcome {
        let event = increment(&mut self.events, &mut self.overflow);
        if entry.iar & 0x3ff != 30 {
            return Outcome::NonPpi;
        }
        let Some(arm) = self.current else { return Outcome::Stale };
        if seq != arm.seq {
            return Outcome::Stale;
        }
        self.current = None;
        if !idle || !matches!(arm.kind, ArmKind::Refill | ArmKind::WakeRefill) {
            return Outcome::Other;
        }

        // ISTATUS may change between samples. ENABLE/IMASK and the comparator
        // must match the intended arm; all independently sampled brackets must order.
        let invalid = arm.cval != arm.due || entry.cval != arm.due ||
            arm.ctl & 3 != 1 || entry.ctl & 3 != 1 ||
            arm.before > arm.after || arm.after > entry.earliest || entry.earliest > entry.counter;
        let lag = entry.earliest.checked_sub(arm.due);
        let outcome = if invalid {
            increment(&mut self.invalid, &mut self.overflow);
            Outcome::Invalid
        } else if let Some(ticks) = lag {
            increment(&mut self.scored, &mut self.overflow);
            self.max_lag = self.max_lag.max(ticks);
            if ticks <= limit {
                return Outcome::Scored;
            }
            increment(&mut self.late, &mut self.overflow);
            Outcome::Late
        } else {
            increment(&mut self.early, &mut self.overflow);
            Outcome::Early
        };
        if self.first.is_none() {
            if let Some(event) = event {
                self.first = Some(Violation {
                    outcome, event, arm, entry,
                    lag: if outcome == Outcome::Late { lag } else { None },
                });
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: u64 = 1_953_125;

    fn entry(due: u64, at: u64) -> Entry {
        Entry { earliest: at, cval: due, ctl: 5, counter: at,
                spsr: 5, elr: 0x40201488, iar: 30 }
    }

    fn refill(health: &mut Health, due: u64, at: u64) -> Outcome {
        let seq = health.arm(ArmKind::Refill, due, 0, 0, due, 1);
        health.observe(seq, entry(due, at), true, LIMIT)
    }

    #[test]
    fn original_misses_survive_full_detail_buffer() {
        let mut h = Health::new();
        let mut retained = [None; 1];
        let mut dropped = 0;
        for (i, (due, at)) in [(10, 10), (328667725, 332869281),
                                (594305368, 600792436)].into_iter().enumerate() {
            let result = refill(&mut h, due, at);
            if let Some(row) = retained.get_mut(i) {
                *row = Some(result);
            } else {
                dropped += 1;
            }
        }
        assert_eq!(retained, [Some(Outcome::Scored)]);
        assert_eq!(dropped, 2);
        assert_eq!((h.events, h.scored, h.late, h.max_lag), (3, 3, 2, 6487068));
        let first = h.first.unwrap();
        assert_eq!((first.outcome, first.event, first.arm.seq), (Outcome::Late, 2, 2));
        assert_eq!((first.arm.due, first.entry.earliest, first.entry.elr),
                   (328667725, 332869281, 0x40201488));
        assert_eq!(first.lag, Some(4201556));
        assert_eq!(first.lag.unwrap() - LIMIT, 2248431);
        assert_eq!(h.max_lag - LIMIT, 4533943);
    }

    #[test]
    fn thresholds_and_counter_endpoints() {
        let mut h = Health::new();
        assert_eq!(h.scored, 0);
        assert_eq!(refill(&mut h, u64::MAX, u64::MAX), Outcome::Scored);
        assert_eq!(refill(&mut h, u64::MAX - LIMIT, u64::MAX), Outcome::Scored);
        assert_eq!(refill(&mut h, u64::MAX - LIMIT - 1, u64::MAX), Outcome::Late);
        assert_eq!(refill(&mut h, 10, 9), Outcome::Early);
        assert_eq!((h.scored, h.late, h.early, h.max_lag), (3, 1, 1, LIMIT + 1));
        let mut early = Health::new();
        assert_eq!(refill(&mut early, u64::MAX, 0), Outcome::Early);
        assert_eq!(early.first.unwrap().lag, None);
    }

    #[test]
    fn identity_and_non_refill_events() {
        let mut h = Health::new();
        let e = entry(20, 21);
        for kind in [ArmKind::Wake, ArmKind::Grant, ArmKind::Horizon, ArmKind::Terminal,
                     ArmKind::Other] {
            let seq = h.arm(kind, 20, 0, 1, 20, 1);
            assert_eq!(h.observe(seq, e, true, LIMIT), Outcome::Other);
        }
        let active = h.arm(ArmKind::Refill, 20, 0, 1, 20, 1);
        assert_eq!(h.observe(active, e, false, LIMIT), Outcome::Other);
        let seq = h.arm(ArmKind::WakeRefill, 20, 0, 1, 20, 1);
        assert_eq!(h.observe(seq, Entry { iar: 27, ..e }, true, LIMIT), Outcome::NonPpi);
        assert_eq!(h.observe(active, e, true, LIMIT), Outcome::Stale);
        assert_eq!(h.observe(seq, e, true, LIMIT), Outcome::Scored);
        assert_eq!(h.observe(seq, e, true, LIMIT), Outcome::Stale);
        assert_eq!((h.scored, h.max_lag, h.first), (1, 1, None));
    }

    #[test]
    fn invalid_measurement_preserves_first_operands() {
        let mut h = Health::new();
        let seq = h.arm(ArmKind::Refill, 20, 8, 9, 20, 1);
        let e = Entry { ctl: 3, ..entry(20, 22) };
        assert_eq!(h.observe(seq, e, true, LIMIT), Outcome::Invalid);
        let first = h.first.unwrap();
        assert_eq!(first.entry, e);
        assert_eq!(first.arm.before, 8);
        assert_eq!((first.lag, h.invalid, h.scored), (None, 1, 0));
        assert_eq!(refill(&mut h, 20, 20 + LIMIT + 10), Outcome::Late);
        assert_eq!(h.first, Some(first));
    }

    #[test]
    fn saturated_counters_never_wrap_or_reissue_identity() {
        let mut h = Health::new();
        h.scored = u64::MAX;
        h.late = u64::MAX;
        assert_eq!(refill(&mut h, 0, u64::MAX), Outcome::Late);
        assert!(h.overflow);
        assert_eq!((h.scored, h.late, h.max_lag), (u64::MAX, u64::MAX, u64::MAX));
        let first = h.first;
        h.events = u64::MAX;
        h.arms = u64::MAX;
        assert_eq!(h.arm(ArmKind::Refill, 0, 0, 0, 0, 1), 0);
        assert_eq!(h.observe(0, entry(0, 0), true, LIMIT), Outcome::Stale);
        assert_eq!((h.arms, h.events, h.first), (u64::MAX, u64::MAX, first));
    }
}
