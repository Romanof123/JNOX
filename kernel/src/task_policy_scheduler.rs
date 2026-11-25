//! Fixed-priority periodic jobs on a monotonic integer clock. Task indices
//! identify individual contexts, including siblings in the same application.
//! Both deadline and watchdog intervals include their final tick.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub priority: u8,
    pub period: u64,
    pub deadline: u64,
    pub watchdog: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Release,
    Complete,
    DeadlineMiss,
    Watchdog,
    Fault(i64),
    Exit,
    Sleep,
    Wake,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub task: usize,
    pub at: u64,
    pub kind: Kind,
}

#[derive(Clone, Copy)]
enum Phase {
    Waiting(Option<u64>),
    Ready,
    Sleeping(u64),
    Terminal(Kind),
}

#[derive(Clone, Copy)]
struct Task {
    config: Config,
    phase: Phase,
    release: u64,
    miss_at: Option<u64>,
    watchdog_at: Option<u64>,
    misses: u64,
}

fn first_late(base: u64, interval: u64) -> Option<u64> {
    base.checked_add(interval)?.checked_add(1)
}

pub struct Scheduler<const N: usize> {
    tasks: [Task; N],
    queue: [usize; N],
    ready: usize,
}

impl<const N: usize> Scheduler<N> {
    const UNUSED: Task = Task {
        config: Config {
            priority: 0,
            period: 0,
            deadline: 0,
            watchdog: None,
        },
        phase: Phase::Waiting(None),
        release: 0,
        miss_at: None,
        watchdog_at: None,
        misses: 0,
    };

    pub const EMPTY: Self = Self {
        tasks: [Self::UNUSED; N],
        queue: [0; N],
        ready: 0,
    };

    /// Initialize static task storage one entry at a time. Invalid input leaves
    /// the scheduler empty, so a partial configuration can never dispatch.
    pub fn reset(&mut self, epoch: u64, mut config_for_id: impl FnMut(usize) -> Config) -> bool {
        self.ready = 0;
        for id in 0..N {
            let config = config_for_id(id);
            if config.priority == 0
                || config.period == 0
                || config.deadline == 0
                || config.watchdog == Some(0)
            {
                self.tasks.fill(Self::UNUSED);
                return false;
            }
            self.tasks[id] = Task {
                config,
                phase: Phase::Waiting(Some(epoch)),
                release: epoch,
                miss_at: None,
                misses: 0,
                // Continuous supervision includes the interval between jobs.
                watchdog_at: config.watchdog.and_then(|w| first_late(epoch, w)),
            };
        }
        true
    }

    pub fn new(configs: [Config; N], epoch: u64) -> Option<Self> {
        let mut scheduler = Self::EMPTY;
        if scheduler.reset(epoch, |id| configs[id]) {
            Some(scheduler)
        } else {
            None
        }
    }

    fn enqueue(&mut self, id: usize) {
        self.queue[self.ready] = id;
        self.ready += 1;
    }

    fn remove(&mut self, id: usize) {
        if let Some(i) = self.queue[..self.ready].iter().position(|&x| x == id) {
            self.queue.copy_within(i + 1..self.ready, i);
            self.ready -= 1;
        }
    }

    pub fn pick(&self) -> Option<usize> {
        let mut winner = None;
        let mut best = 0;
        for &id in &self.queue[..self.ready] {
            let priority = self.tasks[id].config.priority;
            if priority > best {
                winner = Some(id);
                best = priority;
            }
        }
        winner
    }

    pub fn yield_now(&mut self, id: usize) -> bool {
        if !self.runnable(id) {
            return false;
        }
        self.remove(id);
        self.enqueue(id);
        true
    }

    fn runnable(&self, id: usize) -> bool {
        matches!(self.tasks.get(id).map(|t| t.phase), Some(Phase::Ready))
    }

    pub fn terminal_reason(&self, id: usize) -> Option<Kind> {
        match self.tasks.get(id)?.phase {
            Phase::Terminal(kind) => Some(kind),
            _ => None,
        }
    }

    pub fn is_terminal(&self, id: usize) -> bool {
        self.terminal_reason(id).is_some()
    }
    pub fn all_terminal(&self) -> bool {
        self.tasks
            .iter()
            .all(|t| matches!(t.phase, Phase::Terminal(_)))
    }
    pub fn misses(&self, id: usize) -> Option<u64> {
        self.tasks.get(id).map(|t| t.misses)
    }

    // Equal-time events observe a deadline first, then stop a timed-out
    // context before admitting a release or wake. Index orders equal arrivals.
    fn next(&self) -> Option<(u64, u8, usize)> {
        self.tasks
            .iter()
            .enumerate()
            .filter_map(|(id, t)| {
                let arrival = match t.phase {
                    Phase::Waiting(r) => r,
                    Phase::Sleeping(w) => Some(w),
                    Phase::Ready => None,
                    Phase::Terminal(_) => return None,
                };
                [t.miss_at, t.watchdog_at, arrival]
                    .into_iter()
                    .enumerate()
                    .filter_map(|(order, at)| at.map(|at| (at, order as u8, id)))
                    .min()
            })
            .min()
    }

    pub fn next_timer(&self) -> Option<u64> {
        self.next().map(|(at, _, _)| at)
    }

    pub fn advance(&mut self, now: u64, mut emit: impl FnMut(Event)) {
        while let Some((at, order, id)) = self.next() {
            if at > now {
                break;
            }
            let t = &mut self.tasks[id];
            let kind = match order {
                0 => {
                    t.miss_at = None;
                    t.misses = t.misses.saturating_add(1);
                    Kind::DeadlineMiss
                }
                1 => {
                    t.phase = Phase::Terminal(Kind::Watchdog);
                    self.remove(id);
                    Kind::Watchdog
                }
                _ => {
                    let released = matches!(t.phase, Phase::Waiting(_));
                    if released {
                        t.release = at;
                        t.miss_at = first_late(at, t.config.deadline);
                    }
                    t.phase = Phase::Ready;
                    self.enqueue(id);
                    if released {
                        Kind::Release
                    } else {
                        Kind::Wake
                    }
                }
            };
            emit(Event { task: id, at, kind });
        }
    }

    pub fn complete(&mut self, id: usize, now: u64, mut emit: impl FnMut(Event)) -> bool {
        self.advance(now, &mut emit);
        if !self.runnable(id) {
            return false;
        }
        self.remove(id);
        let t = &mut self.tasks[id];
        let period = t.config.period;
        let remainder = (now - t.release) % period;
        // First later nominal release at or after completion; skipped releases
        // never become a burst of backlogged jobs.
        let next = if now == t.release {
            now.checked_add(period)
        } else if remainder == 0 {
            Some(now)
        } else {
            now.checked_add(period - remainder)
        };
        t.phase = Phase::Waiting(next);
        t.miss_at = None;
        t.watchdog_at = t.config.watchdog.and_then(|w| first_late(now, w));
        emit(Event {
            task: id,
            at: now,
            kind: Kind::Complete,
        });
        self.advance(now, emit);
        true
    }

    pub fn sleep(&mut self, id: usize, until: u64, now: u64, mut emit: impl FnMut(Event)) -> bool {
        self.advance(now, &mut emit);
        if !self.runnable(id) || until < now {
            return false;
        }
        self.remove(id);
        self.tasks[id].phase = Phase::Sleeping(until);
        emit(Event {
            task: id,
            at: now,
            kind: Kind::Sleep,
        });
        self.advance(now, emit);
        true
    }

    fn stop(&mut self, id: usize, now: u64, kind: Kind, mut emit: impl FnMut(Event)) -> bool {
        self.advance(now, &mut emit);
        if !self.runnable(id) {
            return false;
        }
        self.remove(id);
        self.tasks[id].phase = Phase::Terminal(kind);
        emit(Event {
            task: id,
            at: now,
            kind,
        });
        true
    }

    pub fn exit(&mut self, id: usize, now: u64, emit: impl FnMut(Event)) -> bool {
        self.stop(id, now, Kind::Exit, emit)
    }

    pub fn fault(&mut self, id: usize, status: i64, now: u64, emit: impl FnMut(Event)) -> bool {
        self.stop(id, now, Kind::Fault(status), emit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            priority: 12,
            period: 10,
            deadline: 6,
            watchdog: Some(12),
        }
    }

    #[test]
    fn exact_intervals_and_nominal_releases() {
        let mut s = Scheduler::new([config()], 10).unwrap();
        assert_eq!(s.next_timer(), Some(10));
        s.advance(10, |_| {});
        assert!(s.complete(0, 16, |_| {}));
        assert_eq!(s.next_timer(), Some(20));
        s.advance(20, |_| {});
        assert!(s.complete(0, 28, |_| {})); // exactly W since last COMPLETE
        assert_eq!(s.misses(0), Some(1));
        assert_eq!(s.next_timer(), Some(30));
        s.advance(30, |_| {});
        assert!(s.exit(0, 30, |_| {}));
        assert_eq!(s.next_timer(), None);
    }

    #[test]
    fn watchdog_during_sleep_orders_deadline_and_isolates_faults() {
        let mut c = config();
        c.watchdog = Some(c.deadline);
        let mut s = Scheduler::new(
            [
                c,
                Config {
                    watchdog: None,
                    ..c
                },
            ],
            0,
        )
        .unwrap();
        assert!(s.sleep(0, 100, 0, |_| {}));
        assert_eq!(s.next_timer(), Some(7));
        let mut log = [None; 3];
        let mut i = 0;
        s.advance(7, |e| {
            log[i] = Some((e.task, e.kind));
            i += 1;
        });
        assert_eq!(
            log,
            [
                Some((0, Kind::DeadlineMiss)),
                Some((1, Kind::DeadlineMiss)),
                Some((0, Kind::Watchdog))
            ]
        );
        assert_eq!(s.pick(), Some(1));
        assert!(s.fault(1, -42, 7, |_| {}));
        assert_eq!(s.terminal_reason(1), Some(Kind::Fault(-42)));
        assert!(s.all_terminal());
        assert!(!s.complete(0, 100, |_| {}));
    }

    #[test]
    fn watchdog_can_expire_in_periodic_wait() {
        let c = Config {
            watchdog: Some(2),
            ..config()
        };
        let mut s = Scheduler::new([c], 40).unwrap();
        assert!(s.complete(0, 40, |_| {}));
        assert_eq!(s.pick(), None);
        assert_eq!(s.next_timer(), Some(43));
        s.advance(50, |_| {});
        assert_eq!(s.terminal_reason(0), Some(Kind::Watchdog));
    }

    #[test]
    fn skips_stale_periods_and_preserves_equal_priority_fifo() {
        let mut configs = [Config {
            watchdog: None,
            ..config()
        }; 70];
        configs[69].priority = 255;
        let mut s = Scheduler::new(configs, 0).unwrap();
        s.advance(0, |_| {});
        assert_eq!(s.pick(), Some(69));
        assert!(s.exit(69, 0, |_| {}));
        assert_eq!(s.pick(), Some(0));
        assert!(s.yield_now(0));
        assert_eq!(s.pick(), Some(1));
        let mut releases = 0;
        assert!(s.complete(1, 1_000_000, |e| {
            if e.kind == Kind::Release {
                releases += 1;
            }
        }));
        assert_eq!(releases, 1);
        assert_eq!(s.pick(), Some(2));
        assert_eq!(s.next_timer(), Some(1_000_007));
    }

    #[test]
    fn no_phantom_expiry_at_end_of_clock() {
        let mut s = Scheduler::new([config()], u64::MAX - 1).unwrap();
        s.advance(u64::MAX, |_| {});
        assert_eq!(s.next_timer(), None);
        assert!(s.complete(0, u64::MAX, |_| {}));
        assert_eq!(s.next_timer(), None);
        assert_eq!(s.pick(), None);
    }
}
