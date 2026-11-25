//! Bounded synchronous IPC for one serialized fixed-priority scheduling domain.
//! Time arguments share one monotonic domain. u64::MAX is an infinite deadline.
//! A caller owns at most one outstanding word; its token also orders arrivals.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub status: i32,
    pub x1: u64,
    pub x2: u64,
}

impl Completion {
    const fn new(status: i32, x1: u64, x2: u64) -> Self {
        Self { status, x1, x2 }
    }
}

#[derive(Clone, Copy)]
struct Request {
    server: usize,
    word: u64,
    deadline: u64,
    token: u64,
    delivered: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReceiveMode {
    Ordinary,
    Broker,
}

#[derive(Clone, Copy)]
struct Slot {
    alive: bool,
    receiving: Option<ReceiveMode>,
    request: Option<Request>,
    // Only a broker receive carries an authenticated caller or deceased peer.
    completion: Option<(Completion, Option<usize>)>,
}

impl Slot {
    const FRESH: Self = Self {
        alive: true,
        receiving: None,
        request: None,
        completion: None,
    };
}

pub struct SyncIpc<const N: usize> {
    routes: [u8; N],
    slots: [Slot; N],
    last_token: u64,
    brokers: u8,
    pending_exits: [u8; N],
}

impl<const N: usize> SyncIpc<N> {
    pub const fn new(routes: [u8; N]) -> Self {
        assert!(N >= 2 && N <= 4);
        Self {
            routes,
            slots: [Slot::FRESH; N],
            last_token: 0,
            brokers: 0,
            pending_exits: [0; N],
        }
    }

    pub const fn with_broker(routes: [u8; N], broker: usize) -> Self {
        Self::new(routes).register_broker(broker)
    }

    pub const fn register_broker(mut self, broker: usize) -> Self {
        assert!(broker < N && self.brokers & (1 << broker) == 0);
        self.brokers |= 1 << broker;
        self
    }

    fn available(&self, pid: usize) -> Result<(), i32> {
        let slot = self.slots.get(pid).filter(|s| s.alive).ok_or(-3)?;
        if slot.request.is_some() || slot.receiving.is_some() || slot.completion.is_some() {
            return Err(-16);
        }
        Ok(())
    }

    pub fn blocked(&self, pid: usize) -> bool {
        self.slots
            .get(pid)
            .is_some_and(|s| s.receiving.is_some() || s.request.is_some())
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.slots
            .iter()
            .filter_map(|s| s.request)
            .map(|r| r.deadline)
            .filter(|&d| d != u64::MAX)
            .min()
    }

    pub fn take(&mut self, pid: usize) -> Option<Completion> {
        self.take_with_caller(pid).map(|(done, _)| done)
    }

    pub fn take_with_caller(&mut self, pid: usize) -> Option<(Completion, Option<usize>)> {
        self.slots.get_mut(pid)?.completion.take()
    }

    fn finish_call(&mut self, pid: usize, status: i32, word: u64) {
        let slot = &mut self.slots[pid];
        slot.request = None;
        slot.completion = Some((Completion::new(status, word, 0), None));
    }

    pub fn advance(&mut self, now: u64) {
        for pid in 0..N {
            if self.slots[pid]
                .request
                .is_some_and(|r| r.deadline != u64::MAX && r.deadline <= now)
            {
                self.finish_call(pid, -110, 0);
            }
        }
    }

    fn match_receive(&mut self, server: usize) {
        let Some(mode) = self.slots[server].receiving else { return };
        // Deliver pending cleanup before further requests. Ordinary RECEIVE
        // continues to select only the oldest undelivered request.
        if mode == ReceiveMode::Broker && self.pending_exits[server] != 0 {
            let dead = self.pending_exits[server].trailing_zeros() as usize;
            self.pending_exits[server] &= !(1 << dead);
            let receiver = &mut self.slots[server];
            receiver.receiving = None;
            receiver.completion = Some((Completion::new(0, 0, 0), Some(dead)));
            return;
        }
        let oldest = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(pid, s)| s.request.map(|r| (pid, r)))
            .filter(|(_, r)| r.server == server && !r.delivered)
            .min_by_key(|(_, r)| r.token);
        if let Some((pid, r)) = oldest {
            self.slots[pid].request.as_mut().unwrap().delivered = true;
            let receiver = &mut self.slots[server];
            receiver.receiving = None;
            let caller = (mode == ReceiveMode::Broker).then_some(pid);
            receiver.completion = Some((Completion::new(0, r.token, r.word), caller));
        }
    }

    pub fn call(
        &mut self,
        caller: usize,
        server: usize,
        word: u64,
        deadline: u64,
        now: u64,
    ) -> Result<(), i32> {
        self.advance(now);
        self.available(caller)?;
        if server >= N {
            return Err(-22);
        }
        if self.routes[caller] & (1 << server) == 0 {
            return Err(-13);
        }
        if !self.slots[server].alive {
            return Err(-32);
        }
        if deadline != u64::MAX && deadline <= now {
            return Err(-110);
        }
        let mut at = server;
        for _ in 0..N {
            if at == caller {
                return Err(-35);
            }
            match self.slots[at].request {
                Some(r) => at = r.server,
                None => break,
            }
        }
        let token = self.last_token.checked_add(1).ok_or(-75)?;
        self.last_token = token;
        self.slots[caller].request = Some(Request {
            server,
            word,
            deadline,
            token,
            delivered: false,
        });
        self.match_receive(server);
        Ok(())
    }

    pub fn receive(&mut self, server: usize, now: u64) -> Result<(), i32> {
        self.start_receive(server, now, ReceiveMode::Ordinary)
    }

    pub fn broker_receive(&mut self, server: usize, now: u64) -> Result<(), i32> {
        if server >= N || self.brokers & (1 << server) == 0 { return Err(-13); }
        self.start_receive(server, now, ReceiveMode::Broker)
    }

    fn start_receive(&mut self, server: usize, now: u64, mode: ReceiveMode) -> Result<(), i32> {
        self.advance(now);
        self.available(server)?;
        self.slots[server].receiving = Some(mode);
        self.match_receive(server);
        Ok(())
    }

    pub fn reply(&mut self, server: usize, token: u64, word: u64, now: u64) -> Result<(), i32> {
        self.advance(now);
        self.available(server)?;
        for pid in 0..N {
            if let Some(r) = self.slots[pid].request {
                if r.token == token && r.delivered {
                    if r.server != server {
                        return Err(-1);
                    }
                    self.finish_call(pid, 0, word);
                    return Ok(());
                }
            }
        }
        Err(-116)
    }

    pub fn exit(&mut self, pid: usize, now: u64) {
        self.advance(now);
        if pid >= N || !self.slots[pid].alive {
            return;
        }
        self.slots[pid] = Slot {
            alive: false,
            ..Slot::FRESH
        };
        for peer in 0..N {
            if self.slots[peer].request.is_some_and(|r| r.server == pid) {
                self.finish_call(peer, -32, 0);
            }
        }
        for broker in 0..N {
            if self.brokers & (1 << broker) == 0 { continue; }
            if pid == broker {
                self.pending_exits[broker] = 0;
            } else if self.slots[broker].alive && self.routes[pid] & (1 << broker) != 0 {
                self.pending_exits[broker] |= 1 << pid;
                self.match_receive(broker);
            }
        }
    }

    /// Reuse a retired slot only after its old execution context is gone.
    /// The domain-wide token sequence survives slot reuse.
    pub fn restart(&mut self, pid: usize) -> Result<(), i32> {
        let slot = self.slots.get(pid).ok_or(-22)?;
        // Reusing a numeric identity must not merge two client deaths into
        // one pending bit or let a fresh request precede its predecessor's cleanup.
        if slot.alive || self.pending_exits.iter().any(|pending| pending & (1 << pid) != 0) {
            return Err(-16);
        }
        self.slots[pid] = Slot::FRESH;
        Ok(())
    }

    pub fn priorities(&self, base: [u8; N]) -> [u8; N] {
        let mut effective = base;
        for _ in 1..N {
            for (caller, slot) in self.slots.iter().enumerate() {
                if let Some(r) = slot.request {
                    effective[r.server] = effective[r.server].max(effective[caller]);
                }
            }
        }
        effective
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FOREVER: u64 = u64::MAX;

    #[test]
    fn broker_uses_the_actual_caller_in_both_receive_orders() {
        let mut ipc = SyncIpc::with_broker([0, 1, 1], 0);
        assert_eq!(ipc.broker_receive(1, 0), Err(-13));
        // Both callers send the same word. Its value happens to name slot 1.
        ipc.broker_receive(0, 0).unwrap();
        ipc.call(2, 0, 1, FOREVER, 1).unwrap();
        let (first, sender) = ipc.take_with_caller(0).unwrap();
        assert_eq!((sender, first.x2), (Some(2), 1));
        assert_ne!(first.x1, 0);
        ipc.call(1, 0, 1, FOREVER, 2).unwrap();
        ipc.broker_receive(0, 3).unwrap();
        let (second, sender) = ipc.take_with_caller(0).unwrap();
        assert_eq!((sender, second.x2), (Some(1), 1));
        assert!(second.x1 > first.x1);
        ipc.reply(0, second.x1, 101, 4).unwrap();
        ipc.reply(0, first.x1, 202, 4).unwrap();
        assert_eq!(ipc.take_with_caller(1), Some((Completion::new(0, 101, 0), None)));
        assert_eq!(ipc.take_with_caller(2), Some((Completion::new(0, 202, 0), None)));
    }

    #[test]
    fn blocked_broker_wakes_once_on_last_authorized_client_exit() {
        let mut ipc = SyncIpc::with_broker([4, 0, 0], 2);
        ipc.broker_receive(2, 0).unwrap();
        ipc.exit(1, 1); // No 1→2 route.
        assert!(ipc.blocked(2));
        assert_eq!(ipc.take_with_caller(2), None);
        ipc.exit(0, 2);
        assert!(!ipc.blocked(2));
        assert_eq!(ipc.take_with_caller(2), Some((Completion::new(0, 0, 0), Some(0))));
        ipc.broker_receive(2, 3).unwrap();
        ipc.exit(0, 4);
        assert!(ipc.blocked(2));
        assert_eq!(ipc.take_with_caller(2), None);
    }

    #[test]
    fn two_brokers_each_receive_only_their_own_routed_deaths() {
        let mut ipc = SyncIpc::with_broker([12, 8, 8, 0], 2).register_broker(3);
        ipc.exit(0, 1);
        ipc.broker_receive(3, 2).unwrap();
        assert_eq!(ipc.take_with_caller(3), Some((Completion::new(0, 0, 0), Some(0))));
        assert_eq!(ipc.restart(0), Err(-16));
        ipc.broker_receive(2, 3).unwrap();
        assert_eq!(ipc.take_with_caller(2), Some((Completion::new(0, 0, 0), Some(0))));
        ipc.restart(0).unwrap();
        ipc.broker_receive(2, 4).unwrap();
        ipc.exit(1, 5);
        assert!(ipc.blocked(2));
        ipc.broker_receive(3, 6).unwrap();
        assert_eq!(ipc.take_with_caller(3), Some((Completion::new(0, 0, 0), Some(1))));
    }

    #[test]
    fn pending_deaths_survive_ordinary_receive_and_retire_reply_tokens() {
        let mut ipc = SyncIpc::with_broker([4, 4, 0], 2);
        ipc.call(0, 2, 61, FOREVER, 0).unwrap();
        ipc.broker_receive(2, 0).unwrap();
        let (request, sender) = ipc.take_with_caller(2).unwrap();
        assert_eq!(sender, Some(0));
        ipc.receive(2, 0).unwrap(); // This receive cannot consume a death.
        ipc.exit(0, 1);
        assert!(ipc.blocked(2));
        assert_eq!(ipc.restart(0), Err(-16));
        ipc.call(1, 2, 18, FOREVER, 2).unwrap();
        let (plain, sender) = ipc.take_with_caller(2).unwrap();
        assert_eq!((plain.x2, sender), (18, None));
        assert_eq!(ipc.reply(2, request.x1, 99, 2), Err(-116));
        ipc.exit(1, 3);
        assert_eq!(ipc.priorities([9, 8, 1]), [9, 8, 1]);
        for pid in [0, 1] {
            ipc.broker_receive(2, 4).unwrap();
            assert_eq!(ipc.take_with_caller(2), Some((Completion::new(0, 0, 0), Some(pid))));
        }
        ipc.restart(0).unwrap();
        ipc.call(0, 2, 7, FOREVER, 5).unwrap();
        ipc.exit(2, 6);
        assert_eq!(ipc.take_with_caller(0), Some((Completion::new(-32, 0, 0), None)));
    }

    #[test]
    fn timeout_wins_equality_and_a_reply_one_tick_earlier_wins() {
        for (at, succeeds) in [(9, true), (10, false), (11, false)] {
            let mut ipc = SyncIpc::new([2, 0]);
            ipc.receive(1, 0).unwrap();
            ipc.call(0, 1, 73, 10, 1).unwrap();
            let msg = ipc.take(1).unwrap();
            assert_eq!(msg.x2, 73);
            assert_eq!(ipc.reply(1, msg.x1, 91, at).is_ok(), succeeds);
            ipc.advance(20);
            assert_eq!(
                ipc.take(0),
                Some(if succeeds {
                    Completion::new(0, 91, 0)
                } else {
                    Completion::new(-110, 0, 0)
                })
            );
            assert_eq!(ipc.take(0), None);
            assert_eq!(ipc.reply(1, msg.x1, 0, 21), Err(-116));
        }
    }

    #[test]
    fn donation_chain_cycle_and_death_unwind() {
        let mut ipc = SyncIpc::new([2, 4, 1]);
        ipc.call(1, 2, 11, FOREVER, 0).unwrap();
        ipc.call(0, 1, 12, FOREVER, 1).unwrap();
        assert_eq!(ipc.priorities([9, 2, 1]), [9, 9, 9]);
        assert_eq!(ipc.call(2, 0, 13, FOREVER, 2), Err(-35));
        ipc.exit(1, 3);
        assert_eq!(ipc.take(0), Some(Completion::new(-32, 0, 0)));
        assert_eq!(ipc.priorities([9, 2, 1]), [9, 2, 1]);
        ipc.receive(2, 4).unwrap();
        assert!(ipc.blocked(2));
    }

    #[test]
    fn arrival_order_rights_token_owner_and_reused_slot() {
        let mut ipc = SyncIpc::new([4, 4, 0]);
        assert_eq!(ipc.call(0, 1, 0, FOREVER, 0), Err(-13));
        ipc.call(1, 2, 11, FOREVER, 0).unwrap();
        ipc.call(0, 2, 12, FOREVER, 0).unwrap();
        ipc.receive(2, 0).unwrap();
        let first = ipc.take(2).unwrap();
        assert_eq!(first.x2, 11);
        ipc.receive(2, 0).unwrap();
        let second = ipc.take(2).unwrap();
        assert_eq!(second.x2, 12);
        ipc.exit(1, 1);
        ipc.restart(1).unwrap();
        assert_eq!(ipc.reply(1, second.x1, 0, 2), Err(-1));
        assert_eq!(ipc.reply(2, first.x1, 0, 2), Err(-116));
        ipc.call(1, 2, 99, FOREVER, 2).unwrap();
        ipc.receive(2, 2).unwrap();
        assert!(ipc.take(2).unwrap().x1 > second.x1);
        ipc.last_token = u64::MAX;
        ipc.reply(2, second.x1, 42, 3).unwrap();
        assert_eq!(ipc.take(0), Some(Completion::new(0, 42, 0)));
        assert_eq!(ipc.call(0, 2, 0, FOREVER, 3), Err(-75));
        assert!(!ipc.blocked(0));
    }
}
