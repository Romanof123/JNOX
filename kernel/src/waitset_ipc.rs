//! Read-only multiplexing of persistent endpoint levels. A single-core masked
//! runtime supplies a snapshot at enrollment and after endpoint transitions.
const CAP: usize = 4;
const MAX_GENERATION: u64 = u64::MAX >> 4;

#[derive(Clone, Copy, Default)]
pub struct Levels {
    pub mailbox: bool,
    pub pulse: bool,
    pub events: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub status: i32,
    pub x1: u64,
    pub x2: u64,
}

#[derive(Clone, Copy)]
enum Source {
    Mailbox,
    Pulse,
    Events(u32),
}

impl Source {
    fn parse(id: u64, mask: u64) -> Result<Self, i32> {
        match (id, mask) {
            (1, 0) => Ok(Self::Mailbox),
            (2, 0) => Ok(Self::Pulse),
            (3, 1..=0xffff_ffff) => Ok(Self::Events(mask as u32)),
            _ => Err(-22),
        }
    }

    fn ready(self, levels: Levels) -> u64 {
        match self {
            Self::Mailbox => u64::from(levels.mailbox),
            Self::Pulse => u64::from(levels.pulse),
            Self::Events(mask) => u64::from(mask & levels.events),
        }
    }
}

#[derive(Clone, Copy)]
struct Registration {
    handle: u64,
    source: Source,
}

#[derive(Clone, Copy)]
struct Set {
    alive: bool,
    generation: u64,
    slots: [Option<Registration>; CAP],
    cursor: usize,
    wait: Option<u64>,
    result: Option<Completion>,
}

impl Set {
    const EMPTY: Self = Self {
        alive: true,
        generation: 0,
        slots: [None; CAP],
        cursor: 0,
        wait: None,
        result: None,
    };

    fn expire(&mut self, now: u64) {
        if self.wait.is_some_and(|d| d != u64::MAX && d <= now) {
            self.wait = None;
            self.result = Some(Completion {
                status: -110,
                x1: 0,
                x2: 0,
            });
        }
    }

    fn select(&mut self, levels: Levels) -> bool {
        for offset in 0..CAP {
            let i = (self.cursor + offset) % CAP;
            if let Some(reg) = self.slots[i] {
                let matched = reg.source.ready(levels);
                if matched != 0 {
                    self.result = Some(Completion {
                        status: 0,
                        x1: reg.handle,
                        x2: matched,
                    });
                    self.cursor = (i + 1) % CAP;
                    self.wait = None;
                    return true;
                }
            }
        }
        false
    }
}

pub struct WaitsetIpc<const N: usize> {
    sets: [Set; N],
}

impl<const N: usize> WaitsetIpc<N> {
    pub const fn new() -> Self {
        assert!(N >= 2 && N <= 3);
        Self {
            sets: [Set::EMPTY; N],
        }
    }

    fn available(&mut self, pid: usize) -> Result<&mut Set, i32> {
        let set = self.sets.get_mut(pid).filter(|s| s.alive).ok_or(-3)?;
        if set.wait.is_some() || set.result.is_some() {
            return Err(-16);
        }
        Ok(set)
    }

    pub fn attach(&mut self, pid: usize, owner: usize, source: u64, mask: u64) -> Result<u64, i32> {
        if pid != owner {
            return Err(-13);
        }
        let source = Source::parse(source, mask)?;
        let set = self.available(pid)?;
        let i = set.slots.iter().position(Option::is_none).ok_or(-28)?;
        if set.generation == MAX_GENERATION {
            return Err(-75);
        }
        set.generation += 1;
        // Two owner bits and two slot bits. Generation survives process exit.
        let handle = (set.generation << 4) | ((i as u64) << 2) | pid as u64;
        set.slots[i] = Some(Registration { handle, source });
        Ok(handle)
    }

    pub fn detach(&mut self, pid: usize, handle: u64) -> Result<(), i32> {
        if (handle & 3) as usize != pid {
            return Err(-13);
        }
        let set = self.available(pid)?;
        let slot = &mut set.slots[((handle >> 2) & 3) as usize];
        if slot.is_some_and(|r| r.handle == handle) {
            *slot = None;
            Ok(())
        } else {
            Err(-116)
        }
    }

    pub fn wait(&mut self, pid: usize, deadline: u64, now: u64, levels: Levels) -> Result<(), i32> {
        self.advance(now);
        let set = self.available(pid)?;
        if deadline != 0 && deadline != u64::MAX && deadline <= now {
            return Err(-110);
        }
        if !set.select(levels) {
            if deadline == 0 {
                return Err(-11);
            }
            set.wait = Some(deadline);
        }
        Ok(())
    }

    pub fn advance(&mut self, now: u64) {
        for set in &mut self.sets {
            set.expire(now);
        }
    }

    pub fn refresh(&mut self, pid: usize, now: u64, levels: Levels) {
        if let Some(set) = self.sets.get_mut(pid) {
            set.expire(now);
            if set.wait.is_some() {
                set.select(levels);
            }
        }
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.sets
            .iter()
            .filter_map(|s| s.wait)
            .filter(|&d| d != u64::MAX)
            .min()
    }
    pub fn take(&mut self, pid: usize) -> Option<Completion> {
        self.sets.get_mut(pid)?.result.take()
    }
    pub fn blocked(&self, pid: usize) -> bool {
        self.sets.get(pid).is_some_and(|s| s.wait.is_some())
    }
    pub fn registered(&self, pid: usize) -> usize {
        self.sets
            .get(pid)
            .map_or(0, |s| s.slots.iter().flatten().count())
    }

    pub fn exit(&mut self, pid: usize, now: u64) {
        self.advance(now);
        if let Some(s) = self.sets.get_mut(pid) {
            *s = Set {
                alive: false,
                generation: s.generation,
                ..Set::EMPTY
            };
        }
    }

    pub fn restart(&mut self, pid: usize) -> Result<(), i32> {
        let set = self.sets.get_mut(pid).ok_or(-22)?;
        if set.alive {
            return Err(-16);
        }
        set.alive = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ready_levels_rotate_without_consumption_and_handles_are_local() {
        let mut w = WaitsetIpc::<3>::new();
        assert_eq!(w.attach(0, 1, 1, 0), Err(-13));
        assert_eq!(w.attach(0, 0, 1, 1), Err(-22));
        assert_eq!(w.attach(0, 0, 3, 1 << 32), Err(-22));
        let regs = [(1, 0), (2, 0), (3, 1), (3, 6)].map(|(s, m)| w.attach(0, 0, s, m).unwrap());
        assert_eq!(w.registered(0), 4);
        assert_eq!(w.attach(0, 0, 2, 0), Err(-28));
        assert_eq!(w.detach(1, regs[0]), Err(-13));
        let levels = Levels {
            mailbox: true,
            pulse: true,
            events: 5,
        };
        for _ in 0..2 {
            for (handle, value) in regs.into_iter().zip([1, 1, 1, 4]) {
                w.wait(0, 0, 1, levels).unwrap();
                assert_eq!(
                    w.take(0),
                    Some(Completion {
                        status: 0,
                        x1: handle,
                        x2: value
                    })
                );
            }
        }
    }

    #[test]
    fn equal_deadline_wins_before_level_wake() {
        for now in [6, 7, 8] {
            let mut w = WaitsetIpc::<2>::new();
            let h = w.attach(1, 1, 1, 0).unwrap();
            w.wait(1, 7, 0, Levels::default()).unwrap();
            assert_eq!(w.next_deadline(), Some(7));
            let ready = Levels {
                mailbox: true,
                ..Levels::default()
            };
            w.refresh(1, now, ready);
            let expected = if now < 7 {
                Completion {
                    status: 0,
                    x1: h,
                    x2: 1,
                }
            } else {
                Completion {
                    status: -110,
                    x1: 0,
                    x2: 0,
                }
            };
            assert_eq!(w.take(1), Some(expected));
            w.refresh(1, now, ready);
            assert_eq!(w.take(1), None);
            w.wait(1, 0, now, ready).unwrap();
            assert_eq!(w.take(1).unwrap().x1, h);
        }
    }

    #[test]
    fn stale_pulse_cannot_report_for_replacement_and_reset_revokes_results() {
        let mut w = WaitsetIpc::<2>::new();
        let old = w.attach(0, 0, 2, 0).unwrap();
        w.detach(0, old).unwrap();
        let new = w.attach(0, 0, 3, 8).unwrap();
        assert_ne!(old, new);
        let other = Levels {
            pulse: true,
            events: 4,
            ..Levels::default()
        };
        assert_eq!(w.wait(0, 0, 1, other), Err(-11));
        w.wait(0, u64::MAX, 1, other).unwrap();
        assert_eq!(w.detach(0, new), Err(-16));
        w.refresh(0, 2, other);
        assert!(w.blocked(0));
        w.refresh(
            0,
            3,
            Levels {
                events: 12,
                ..other
            },
        );
        w.exit(0, 3);
        assert_eq!(w.registered(0), 0);
        assert_eq!(w.take(0), None);
        w.restart(0).unwrap();
        assert_eq!(w.detach(0, new), Err(-116));
        let newest = w.attach(0, 0, 1, 0).unwrap();
        assert_ne!(newest, new);
        w.sets[0].generation = MAX_GENERATION;
        assert_eq!(w.attach(0, 0, 2, 0), Err(-75));
        assert_eq!(w.registered(0), 1);
    }
}
