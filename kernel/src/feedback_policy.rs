//! Per-task CPU accounting for three feedback levels, and proportional ticket selection.
use core::hint::black_box;

/// Each charge ends at or before a scheduling boundary. A yield or sleep leaves
/// this object intact. The caller applies a boost to each task at its boost epoch.
pub(crate) struct FeedbackTask {
    allotments: [u64; 3],
    level: usize,
    left: u64,
}

impl FeedbackTask {
    pub(crate) fn new(allotments: [u64; 3]) -> Option<Self> {
        if allotments.contains(&0) {
            return None;
        }
        Some(Self {
            allotments,
            level: 0,
            left: allotments[0],
        })
    }

    pub(crate) fn priority(&self) -> u8 {
        (self.allotments.len() - 1 - self.level) as u8
    }

    pub(crate) fn remaining(&self) -> u64 {
        self.left
    }

    /// On exhaustion, the caller requeues the task at the tail of its new level.
    pub(crate) fn charge(&mut self, ticks: u64) -> bool {
        if ticks > self.left {
            return false;
        }
        self.left -= ticks;
        if self.left == 0 {
            self.level = (self.level + 1).min(self.allotments.len() - 1);
            self.left = self.allotments[self.level];
        }
        true
    }

    /// Settle delayed interrupts in constant time, including multiple bottom-level quanta.
    #[cfg(jnox_multilevel)]
    pub(crate) fn charge_interval(&mut self, mut service: u64) {
        while self.level < 2 && service >= self.left {
            service -= self.left;
            self.charge(self.left);
        }
        if self.level == 2 && service >= self.left {
            service = (service - self.left) % self.allotments[2];
            self.left = self.allotments[2];
        }
        self.charge(service);
    }

    pub(crate) fn boost(&mut self) {
        self.level = 0;
        self.left = self.allotments[0];
    }
}

/// Index into contiguous intervals for one caller-supplied uniform draw
/// in 0..sum(tickets). A zero ticket count denotes an ineligible task.
pub(crate) fn lottery_winner(tickets: &[u32], mut draw: u64) -> Option<usize> {
    for (id, &weight) in tickets.iter().enumerate() {
        let width = u64::from(weight);
        if draw < width {
            return Some(id);
        }
        draw -= width;
    }
    None
}

pub(crate) fn bench_checks() -> bool {
    let Some(mut task) = FeedbackTask::new(black_box([3, 5, 8])) else {
        return false;
    };
    // Two separate dispatches consume the same top-level allotment. Voluntary
    // yielding between them must not keep a CPU-bound task at the top level.
    if !task.charge(1) || task.remaining() != 2 || !task.charge(2) || task.priority() != 1 {
        return false;
    }
    if task.charge(6) || task.remaining() != 5 || !task.charge(5) || task.priority() != 0 {
        return false;
    }
    if !task.charge(8) || task.priority() != 0 || task.remaining() != 8 {
        return false;
    }
    task.boost();
    let weights = black_box([2, 0, 1, 3]);
    let expected = [Some(0), Some(0), Some(2), Some(3), Some(3), Some(3), None];
    task.priority() == 2
        && task.remaining() == 3
        && FeedbackTask::new(black_box([1, 0, 1])).is_none()
        && expected
            .iter()
            .enumerate()
            .all(|(draw, &id)| lottery_winner(&weights, draw as u64) == id)
        && lottery_winner(&black_box([0, 0]), 0).is_none()
}
