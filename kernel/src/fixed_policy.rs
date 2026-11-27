//! Single-core fixed-priority policy. All times use one monotonic counter.
use crate::ready_queue::ReadyQueue;

pub const MAX_TASKS: usize = 4 + cfg!(jnox_native_display_standby) as usize;
const MAX_INTERVAL: u64 = i64::MAX as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompareOverflow;

pub(crate) fn relative_deadline(now: u64, delay: u64) -> Option<u64> {
    if delay > MAX_INTERVAL {
        return None;
    }
    now.checked_add(delay)
}

/// A saturated equal-rank turn rotates only when another task can run.
pub(crate) fn rotate_expired(remaining: u64, equal_peer_ready: impl FnOnce() -> bool) -> bool {
    remaining == 0 && equal_peer_ready()
}

#[derive(Clone, Copy)]
pub struct Config {
    pub priority: u8,
    pub delay: u64,
}

#[derive(Clone, Copy)]
pub enum Mode {
    Fifo,
    RoundRobin(u64),
}

#[derive(Clone, Copy)]
pub enum Action {
    Poll,
    #[cfg(any(jnox_native_sync_ipc, jnox_native_mailbox_ipc, jnox_native_notify_ipc, jnox_native_waitset, jnox_native_pipe_ipc))]
    BlockIpc,
    #[cfg(jnox_linux_source_net_rx_irq)]
    BlockIrq,
    #[cfg(jnox_linux_source_net_rx_irq)]
    WakeIrq(usize),
    Yield,
    Sleep(u64),
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Running,
    #[cfg(any(jnox_native_sync_ipc, jnox_native_mailbox_ipc, jnox_native_notify_ipc, jnox_native_waitset, jnox_native_pipe_ipc))]
    BlockedIpc,
    #[cfg(jnox_linux_source_net_rx_irq)]
    BlockedIrq,
    #[cfg(any(jnox_native_shell, jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
    Dormant,
    Sleeping(u64),
    Exited,
}

#[derive(Clone, Copy)]
struct Task {
    priority: u8,
    state: State,
    remaining: u64,
    service: u64,
}

pub struct Fixed {
    tasks: [Task; MAX_TASKS],
    count: usize,
    ready: ReadyQueue<MAX_TASKS>,
    owner: Option<usize>,
    quantum: Option<u64>,
    now: u64,
}

impl Fixed {
    pub fn new(tasks: &[Config], mode: Mode, now: u64) -> Option<Self> {
        Self::initial(tasks, mode, now, 0)
    }

    #[cfg(jnox_native_shell)]
    pub(crate) fn new_shell(tasks: &[Config], mode: Mode, now: u64) -> Option<Self> {
        if tasks.len() != 3 { return None; }
        Self::initial(tasks, mode, now, 0b110)
    }

    #[cfg(any(jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
    pub(crate) fn new_with_standby(tasks: &[Config], mode: Mode, now: u64) -> Option<Self> {
        let count = if cfg!(jnox_native_display_standby) { 5 } else { 3 };
        if tasks.len() != count || !matches!(mode, Mode::Fifo) { return None; }
        Self::initial(tasks, mode, now, 1 << 2)
    }

    fn initial(tasks: &[Config], mode: Mode, now: u64, dormant_mask: u8) -> Option<Self> {
        if tasks.len() > MAX_TASKS {
            return None;
        }
        let quantum = match mode {
            Mode::Fifo => None,
            Mode::RoundRobin(ticks) if ticks == 0 || ticks > MAX_INTERVAL => return None,
            Mode::RoundRobin(ticks) => Some(ticks),
        };
        let vacant = Task {
            priority: 0,
            state: State::Exited,
            remaining: 0,
            service: 0,
        };
        let mut policy = Self {
            tasks: [vacant; MAX_TASKS],
            count: tasks.len(),
            ready: ReadyQueue::new(),
            owner: None,
            quantum,
            now,
        };
        for (id, config) in tasks.iter().enumerate() {
            policy.tasks[id].priority = config.priority;
            let runnable = dormant_mask & (1 << id) == 0;
            if runnable {
                policy.tasks[id].state = State::Sleeping(relative_deadline(now, config.delay)?);
            }
            #[cfg(any(jnox_native_shell, jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
            if !runnable { policy.tasks[id].state = State::Dormant; }
        }
        policy.wake_due();
        policy.select();
        policy.deadline().ok()?;
        Some(policy)
    }

    #[cfg(any(jnox_native_shell, jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
    pub(crate) fn dormant(&self, id: usize) -> bool {
        id < self.count && self.tasks[id].state == State::Dormant
    }

    #[cfg(any(jnox_native_shell, jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
    pub(crate) fn launch(&mut self, id: usize) {
        assert!(self.dormant(id));
        self.join_tail(id);
    }

    #[cfg(any(jnox_native_shell, jnox_native_uart_rx_standby, jnox_native_display_standby, jnox_linux_source_net_restart))]
    pub(crate) fn dismiss(&mut self, id: usize) {
        assert!(self.dormant(id));
        self.tasks[id].state = State::Exited;
    }

    pub fn current(&self) -> Option<usize> {
        self.owner
    }

    pub fn finished(&self) -> bool {
        self.tasks[..self.count]
            .iter()
            .all(|task| task.state == State::Exited)
    }

    pub fn service(&self, id: usize) -> Option<u64> {
        Some(self.tasks[..self.count].get(id)?.service)
    }

    #[cfg(jnox_native_sync_ipc)]
    pub(crate) fn ipc_priorities(&mut self, ranks: &[u8]) {
        assert_eq!(self.count, ranks.len());
        for (task, &rank) in self.tasks[..self.count].iter_mut().zip(ranks) {
            task.priority = rank;
        }
    }

    #[cfg(any(jnox_native_sync_ipc, jnox_native_mailbox_ipc, jnox_native_notify_ipc, jnox_native_waitset, jnox_native_pipe_ipc))]
    pub(crate) fn ipc_wake(&mut self, id: usize) {
        assert!(self.tasks[id].state == State::BlockedIpc);
        self.join_tail(id);
    }

    fn has_peer(&self, id: usize) -> bool {
        self.tasks[..self.count]
            .iter()
            .any(|task| task.state == State::Ready && task.priority == self.tasks[id].priority)
    }

    pub(crate) fn next_wake(&self) -> Option<u64> {
        self.tasks[..self.count]
            .iter()
            .filter_map(|task| match task.state {
                State::Sleeping(at) => Some(at),
                _ => None,
            })
            .min()
    }

    pub(crate) fn expiry(&self) -> Result<Option<u64>, CompareOverflow> {
        self
            .owner
            .filter(|&id| self.quantum.is_some() && self.has_peer(id))
            .map(|id| {
                self.now
                    .checked_add(self.tasks[id].remaining)
                    .ok_or(CompareOverflow)
            })
            .transpose()
    }

    pub fn deadline(&self) -> Result<Option<u64>, CompareOverflow> {
        Ok(self.next_wake().into_iter().chain(self.expiry()?).min())
    }

    #[cfg(jnox_native_el0)]
    pub(crate) fn remaining(&self, id: usize) -> u64 { self.tasks[id].remaining }

    /// Decoding a decision must not consume or reorder the live ready queue.
    #[cfg(jnox_native_el0)]
    pub(crate) fn ready_order(&self, output: &mut [u64]) {
        let mut len = 0;
        for &id in self.ready.entries() {
            let mut at = len;
            while at > 0 && self.tasks[output[at - 1] as usize].priority < self.tasks[id].priority {
                output[at] = output[at - 1];
                at -= 1;
            }
            output[at] = id as u64;
            len += 1;
        }
    }

    fn join_tail(&mut self, id: usize) {
        self.tasks[id].state = State::Ready;
        self.tasks[id].remaining = self.quantum.unwrap_or(0);
        assert!(self.ready.push_back(id));
    }

    fn wake_due(&mut self) {
        loop {
            let due = self.tasks[..self.count]
                .iter()
                .enumerate()
                .filter_map(|(id, task)| match task.state {
                    State::Sleeping(at) if at <= self.now => Some((at, id)),
                    _ => None,
                })
                .min();
            let Some((_, id)) = due else { return };
            self.join_tail(id);
        }
    }

    fn select(&mut self) {
        if let Some(id) = self.owner {
            if self.quantum.is_some() && rotate_expired(self.tasks[id].remaining, || self.has_peer(id)) {
                self.join_tail(id);
            } else {
                // Preserve the turn across higher-priority interruptions.
                self.tasks[id].state = State::Ready;
                assert!(self.ready.push_front(id));
            }
        }
        self.owner = self.ready.pop_highest(|id| self.tasks[id].priority);
        if let Some(id) = self.owner {
            self.tasks[id].state = State::Running;
        }
    }

    /// None rejects the event before mutation. An overflowing sleep returns
    /// Some(false), with elapsed time and any simultaneous releases processed.
    pub fn event(&mut self, now: u64, action: Action) -> Option<bool> {
        let elapsed = now.checked_sub(self.now)?;
        match action {
            Action::Poll => {}
            #[cfg(jnox_linux_source_net_rx_irq)]
            Action::WakeIrq(id) => {
                if id >= self.count || self.tasks[id].state != State::BlockedIrq { return None; }
            }
            _ if self.owner.is_none() => return None,
            _ => {}
        }
        self.now = now;
        if let Some(id) = self.owner {
            let task = &mut self.tasks[id];
            // Disjoint intervals sum to at most (now - boot_epoch).
            task.service += elapsed;
            task.remaining = task.remaining.saturating_sub(elapsed);
        }
        self.wake_due();
        #[cfg(jnox_linux_source_net_rx_irq)]
        if let Action::WakeIrq(id) = action {
            // Sleepers already due and existing ready peers keep their FIFO
            // turns. An equal-priority running peer also keeps its turn.
            self.join_tail(id);
        }
        let mut accepted = true;
        if let Some(id) = self.owner {
            match action {
                Action::Poll => {}
                #[cfg(jnox_linux_source_net_rx_irq)]
                Action::WakeIrq(_) => {}
                #[cfg(jnox_linux_source_net_rx_irq)]
                Action::BlockIrq => {
                    self.tasks[id].state = State::BlockedIrq;
                    self.owner = None;
                }
                #[cfg(any(jnox_native_sync_ipc, jnox_native_mailbox_ipc, jnox_native_notify_ipc, jnox_native_waitset, jnox_native_pipe_ipc))]
                Action::BlockIpc => {
                    self.tasks[id].state = State::BlockedIpc;
                    self.owner = None;
                }
                Action::Yield | Action::Sleep(0) => {
                    self.owner = None;
                    self.join_tail(id);
                }
                Action::Sleep(delay) => {
                    if let Some(at) = relative_deadline(now, delay) {
                        self.tasks[id].state = State::Sleeping(at);
                        self.owner = None;
                    } else {
                        accepted = false;
                    }
                }
                Action::Exit => {
                    self.tasks[id].state = State::Exited;
                    self.owner = None;
                }
            }
        }
        self.select();
        Some(accepted)
    }
}
