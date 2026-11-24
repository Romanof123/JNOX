//! Bounded, stable ready queue for task IDs in `0..N`.

pub(crate) struct ReadyQueue<const N: usize> {
    ids: [usize; N],
    len: usize,
}

impl<const N: usize> ReadyQueue<N> {
    pub(crate) const fn new() -> Self {
        Self {
            ids: [0; N],
            len: 0,
        }
    }

    #[cfg(jnox_multilevel_trace)]
    pub(crate) fn entries(&self) -> &[usize] { &self.ids[..self.len] }

    fn can_insert(&self, id: usize) -> bool {
        self.len < N && id < N && !self.ids[..self.len].contains(&id)
    }

    /// Newly ready tasks and tasks that yield join the back of their priority.
    pub(crate) fn push_back(&mut self, id: usize) -> bool {
        if !self.can_insert(id) {
            return false;
        }
        self.ids[self.len] = id;
        self.len += 1;
        true
    }

    /// A preempted task keeps its turn ahead of waiting peers.
    pub(crate) fn push_front(&mut self, id: usize) -> bool {
        if !self.can_insert(id) {
            return false;
        }
        for index in (0..self.len).rev() {
            self.ids[index + 1] = self.ids[index];
        }
        self.ids[0] = id;
        self.len += 1;
        true
    }

    /// Removes the first best ID. Return true only for a strict preference.
    pub(crate) fn pop_preferred(&mut self, prefer: impl Fn(usize, usize) -> bool) -> Option<usize> {
        self.pop_ranked(Some, prefer)
    }

    pub(crate) fn pop_highest(&mut self, priority: impl Fn(usize) -> u8) -> Option<usize> {
        self.pop_ranked(|id| Some(priority(id)), |candidate, best| candidate > best)
    }

    /// Takes one snapshot per queued ID. An ID without a rank stays in place.
    pub(crate) fn pop_ranked<T: Copy>(
        &mut self,
        rank: impl Fn(usize) -> Option<T>,
        prefer: impl Fn(T, T) -> bool,
    ) -> Option<usize> {
        let mut winner = None;
        for (index, &id) in self.ids[..self.len].iter().enumerate() {
            let Some(candidate) = rank(id) else {
                continue;
            };
            if winner.is_none_or(|(_, best)| prefer(candidate, best)) {
                winner = Some((index, candidate));
            }
        }
        let (best, _) = winner?;
        let selected = self.ids[best];
        self.len -= 1;
        for index in best..self.len {
            self.ids[index] = self.ids[index + 1];
        }
        Some(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::ReadyQueue;

    #[test]
    fn yield_moves_to_tail_even_when_ids_arrive_in_reverse_order() {
        let mut queue = ReadyQueue::<3>::new();
        for id in [2, 1, 0] {
            assert!(queue.push_back(id));
        }
        assert_eq!(queue.pop_highest(|_| 0), Some(2));
        assert!(queue.push_back(2));
        for id in [1, 0, 2] {
            assert_eq!(queue.pop_highest(|_| 0), Some(id));
        }
        assert_eq!(queue.pop_highest(|_| 0), None);
    }

    #[test]
    fn preempted_task_precedes_peers_and_higher_priorities_still_win() {
        let mut queue = ReadyQueue::<5>::new();
        let priority = |id: usize| [0, 4, u8::MAX, 4, 4][id];
        for id in [0, 4, 2, 1] {
            assert!(queue.push_back(id));
        }
        assert!(queue.push_front(3));
        for id in [2, 3, 4, 1, 0] {
            assert_eq!(queue.pop_highest(priority), Some(id));
        }
    }

    #[test]
    fn rejected_insertions_preserve_contents_and_order() {
        let mut queue = ReadyQueue::<3>::new();
        assert!(queue.push_back(2));
        assert!(!queue.push_front(2));
        assert!(!queue.push_back(2));
        assert!(!queue.push_front(3));
        assert!(!queue.push_back(usize::MAX));
        assert!(queue.push_front(1));
        assert!(queue.push_back(0));
        assert!(!queue.push_front(0));
        assert!(!queue.push_back(1));
        for id in [1, 2, 0] {
            assert_eq!(queue.pop_highest(|_| 1), Some(id));
        }
        assert_eq!(queue.pop_highest(|_| 1), None);

        let mut empty = ReadyQueue::<0>::new();
        assert!(!empty.push_back(0));
        assert!(!empty.push_front(0));
        assert_eq!(empty.pop_highest(|_| unreachable!()), None);
    }
}
