//! Pure decision logic used by both the masked EL1 watchdog and its host vectors.
#[derive(Default)]
pub(crate) struct CompletionWatchdog {
    pub(crate) last: Option<u64>,
    pub(crate) due: Option<u64>,
    pub(crate) fault: bool,
    observed: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Decision { Unchanged, Primed, Renewed, Expired, Clock, Unrepresentable, Latched }

impl CompletionWatchdog {
    pub(crate) fn stop(&mut self) {
        self.fault = true;
        self.due = None;
    }

    pub(crate) fn poll(&mut self, now: u64) -> Decision {
        if self.fault { return Decision::Latched; }
        let prior = self.observed.replace(now);
        let bad_clock = prior.is_some_and(|p| now.checked_sub(p).is_none_or(|d| d > i64::MAX as u64));
        let result = if bad_clock { Decision::Clock }
            else if self.due.is_some_and(|due| now >= due) { Decision::Expired }
            else { return Decision::Unchanged };
        self.stop();
        result
    }

    pub(crate) fn complete(&mut self, now: u64, hz: u64) -> Decision {
        let old = self.poll(now);
        if old != Decision::Unchanged { return old; }
        // Boot rejects zero CNTFRQ. Fail closed on any invalid frequency here too.
        let allowed = (u128::from(hz) * 5 / 1000) as u64;
        let next = now.checked_add(allowed).and_then(|end| end.checked_add(1));
        if hz == 0 || next.is_none() {
            self.stop();
            return Decision::Unrepresentable;
        }
        let prior = self.last.replace(now);
        self.due = next;
        if prior.is_some() { Decision::Renewed } else { Decision::Primed }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionWatchdog, Decision::*};

    #[test]
    fn physical_rate_allows_exactly_five_ms() {
        for (gap, result) in [(312_499, Renewed), (312_500, Renewed),
                              (312_501, Expired), (312_502, Expired)] {
            let mut w = CompletionWatchdog::default();
            assert_eq!(w.complete(100, 62_500_000), Primed);
            assert_eq!(w.due, Some(312_601));
            assert_eq!(w.complete(100 + gap, 62_500_000), result);
            if result == Expired {
                assert_eq!((w.last,w.due,w.fault), (Some(100),None,true));
                assert_eq!(w.complete(101 + gap, 62_500_000), Latched);
            } else {
                assert_eq!(w.due, Some(312_601 + gap));
            }
        }
    }

    #[test]
    fn silent_poll_and_low_or_fractional_tick_rates() {
        for (hz, allowed) in [(1,0), (199,0), (200,1), (999,4), (1001,5)] {
            let mut w = CompletionWatchdog::default();
            assert_eq!(w.complete(10, hz), Primed);
            assert_eq!(w.poll(10 + allowed), Unchanged);
            assert_eq!(w.poll(11 + allowed), Expired);
            assert_eq!(w.complete(11 + allowed, hz), Latched);
        }
        assert_eq!(CompletionWatchdog::default().complete(1,0), Unrepresentable);
    }

    #[test]
    fn absolute_deadline_must_be_representable() {
        let mut w = CompletionWatchdog::default();
        assert_eq!(w.complete(u64::MAX - 6, 1000), Primed);
        assert_eq!(w.due, Some(u64::MAX));
        assert_eq!(w.complete(u64::MAX - 5, 1000), Unrepresentable);
        assert_eq!((w.last,w.due,w.fault), (Some(u64::MAX - 6),None,true));
        assert_eq!(CompletionWatchdog::default().complete(u64::MAX,1), Unrepresentable);
        let mut wide = CompletionWatchdog::default();
        assert_eq!(wide.complete(0, u64::MAX), Primed);
        assert_eq!(wide.due, Some(u64::MAX / 200 + 1));
    }

    #[test]
    fn backwards_and_ambiguous_elapsed_time_latch() {
        let mut w = CompletionWatchdog::default();
        assert_eq!(w.complete(7,1000), Primed);
        assert_eq!(w.complete(6,1000), Clock);
        assert_eq!(w.last, Some(7));
        assert_eq!(w.poll(8), Latched);
        for (delta, result) in [(i64::MAX as u64, Unchanged), (1u64 << 63, Clock)] {
            let mut w = CompletionWatchdog::default();
            assert_eq!(w.poll(0), Unchanged);
            assert_eq!(w.poll(delta), result);
        }
    }
}
