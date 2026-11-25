//! Four copied words per process-owned endpoint. All operations are serialized
//! by the single-core runtime. Times use absolute monotonic nanoseconds.
const CAPACITY: usize = 4;
const FOREVER: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub status: i32,
    pub x1: u64,
    pub x2: u64,
}

impl Completion {
    const fn new(status: i32, word: u64) -> Self {
        Self {
            status,
            x1: word,
            x2: 0,
        }
    }
}

#[derive(Clone, Copy)]
enum Wait {
    None,
    Receive {
        deadline: u64,
    },
    Send {
        owner: usize,
        word: u64,
        deadline: u64,
    },
}

#[derive(Clone, Copy)]
struct Process {
    alive: bool,
    wait: Wait,
    result: Option<Completion>,
}

#[derive(Clone, Copy)]
struct Mailbox<const N: usize> {
    words: [u64; CAPACITY],
    head: usize,
    len: usize,
    peak: usize,
    // Separate arrival order for still-uncommitted sends.
    senders: [usize; N],
    waiting: usize,
}

impl<const N: usize> Mailbox<N> {
    const EMPTY: Self = Self {
        words: [0; CAPACITY],
        head: 0,
        len: 0,
        peak: 0,
        senders: [0; N],
        waiting: 0,
    };

    fn push(&mut self, word: u64) {
        debug_assert!(self.len < CAPACITY);
        self.words[(self.head + self.len) % CAPACITY] = word;
        self.len += 1;
        self.peak = self.peak.max(self.len);
    }

    fn pop(&mut self) -> u64 {
        debug_assert!(self.len != 0);
        let word = self.words[self.head];
        self.words[self.head] = 0;
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        word
    }

    fn remove_sender(&mut self, sender: usize) {
        let pos = self.senders[..self.waiting]
            .iter()
            .position(|&p| p == sender)
            .expect("pending sender belongs to endpoint queue");
        self.senders.copy_within(pos + 1..self.waiting, pos);
        self.waiting -= 1;
    }
}

pub struct MailboxIpc<const N: usize> {
    routes: [u8; N],
    tasks: [Process; N],
    endpoints: [Mailbox<N>; N],
}

impl<const N: usize> MailboxIpc<N> {
    pub const fn new(routes: [u8; N]) -> Self {
        assert!(N >= 2 && N <= 3);
        Self {
            routes,
            tasks: [Process {
                alive: true,
                wait: Wait::None,
                result: None,
            }; N],
            endpoints: [Mailbox::EMPTY; N],
        }
    }

    fn available(&self, pid: usize) -> Result<(), i32> {
        let task = self.tasks.get(pid).filter(|p| p.alive).ok_or(-3)?;
        if !matches!(task.wait, Wait::None) || task.result.is_some() {
            return Err(-16);
        }
        Ok(())
    }

    fn complete(&mut self, pid: usize, status: i32, word: u64) {
        self.tasks[pid].wait = Wait::None;
        self.tasks[pid].result = Some(Completion::new(status, word));
    }

    pub fn take(&mut self, pid: usize) -> Option<Completion> {
        self.tasks.get_mut(pid)?.result.take()
    }

    pub fn blocked(&self, pid: usize) -> bool {
        self.tasks
            .get(pid)
            .is_some_and(|p| !matches!(p.wait, Wait::None))
    }

    pub fn occupancy(&self, owner: usize) -> usize {
        self.endpoints.get(owner).map_or(0, |e| e.len)
    }

    pub fn high_water(&self, owner: usize) -> usize {
        self.endpoints.get(owner).map_or(0, |e| e.peak)
    }

    fn deadline(wait: Wait) -> Option<u64> {
        match wait {
            Wait::Send { deadline, .. } | Wait::Receive { deadline } if deadline != FOREVER => {
                Some(deadline)
            }
            _ => None,
        }
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.tasks
            .iter()
            .filter_map(|p| Self::deadline(p.wait))
            .min()
    }

    pub fn advance(&mut self, now: u64) {
        for pid in 0..N {
            let wait = self.tasks[pid].wait;
            if Self::deadline(wait).is_some_and(|d| d <= now) {
                if let Wait::Send { owner, .. } = wait {
                    self.endpoints[owner].remove_sender(pid);
                }
                self.complete(pid, -110, 0);
            }
        }
    }

    fn expired(deadline: u64, now: u64) -> bool {
        deadline != 0 && deadline != FOREVER && deadline <= now
    }

    pub fn send(
        &mut self,
        sender: usize,
        owner: usize,
        word: u64,
        deadline: u64,
        now: u64,
    ) -> Result<(), i32> {
        self.send_checked(sender, owner, word, deadline, now, true)
    }

    /// The kernel caller has resolved a live SEND capability for this owner.
    #[cfg(jnox_native_capability_ipc)]
    pub(super) fn send_resolved(
        &mut self, sender: usize, owner: usize, word: u64, deadline: u64, now: u64,
    ) -> Result<(), i32> {
        self.send_checked(sender, owner, word, deadline, now, false)
    }

    fn send_checked(
        &mut self, sender: usize, owner: usize, word: u64, deadline: u64, now: u64,
        require_route: bool,
    ) -> Result<(), i32> {
        self.advance(now);
        self.available(sender)?;
        if owner >= N {
            return Err(-22);
        }
        if require_route && self.routes[sender] & (1 << owner) == 0 {
            return Err(-13);
        }
        if !self.tasks[owner].alive {
            return Err(-32);
        }
        if Self::expired(deadline, now) {
            return Err(-110);
        }
        if matches!(self.tasks[owner].wait, Wait::Receive { .. }) {
            self.complete(owner, 0, word);
        } else if self.endpoints[owner].len < CAPACITY {
            self.endpoints[owner].push(word);
        } else {
            if deadline == 0 {
                return Err(-11);
            }
            let ep = &mut self.endpoints[owner];
            ep.senders[ep.waiting] = sender;
            ep.waiting += 1;
            self.tasks[sender].wait = Wait::Send {
                owner,
                word,
                deadline,
            };
            return Ok(());
        }
        self.complete(sender, 0, 0);
        Ok(())
    }

    pub fn receive(&mut self, owner: usize, deadline: u64, now: u64) -> Result<(), i32> {
        self.advance(now);
        self.available(owner)?;
        if Self::expired(deadline, now) {
            return Err(-110);
        }
        if self.endpoints[owner].len == 0 {
            if deadline == 0 {
                return Err(-11);
            }
            self.tasks[owner].wait = Wait::Receive { deadline };
            return Ok(());
        }
        let ep = &mut self.endpoints[owner];
        let word = ep.pop();
        if ep.waiting > 0 {
            let sender = ep.senders[0];
            let Wait::Send { word: pending, .. } = self.tasks[sender].wait else {
                unreachable!("endpoint queue contains pending senders");
            };
            ep.remove_sender(sender);
            ep.push(pending);
            self.complete(sender, 0, 0);
        }
        self.complete(owner, 0, word);
        Ok(())
    }

    pub fn exit(&mut self, pid: usize, now: u64) {
        self.advance(now);
        if pid >= N || !self.tasks[pid].alive {
            return;
        }
        if let Wait::Send { owner, .. } = self.tasks[pid].wait {
            self.endpoints[owner].remove_sender(pid);
        }
        self.tasks[pid] = Process {
            alive: false,
            wait: Wait::None,
            result: None,
        };
        let endpoint = self.endpoints[pid];
        for &sender in &endpoint.senders[..endpoint.waiting] {
            self.complete(sender, -32, 0);
        }
        // The terminal report retains peak occupancy of the closed endpoint.
        self.endpoints[pid] = Mailbox {
            peak: endpoint.peak,
            ..Mailbox::EMPTY
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fill(m: &mut MailboxIpc<3>) {
        for word in 0..4 {
            m.send(0, 2, word, 0, 0).unwrap();
            assert_eq!(m.take(0), Some(Completion::new(0, 0)));
        }
        assert_eq!(m.occupancy(2), 4);
    }
    fn get(m: &mut MailboxIpc<3>, now: u64) -> u64 {
        m.receive(2, 0, now).unwrap();
        m.take(2).unwrap().x1
    }

    #[test]
    fn expiry_precedes_promotion_and_order_survives_wrap() {
        for now in [9, 10, 11] {
            let mut m = MailboxIpc::new([4, 4, 0]);
            fill(&mut m);
            assert_eq!(m.send(0, 2, 77, 0, 1), Err(-11));
            m.send(1, 2, 55, 10, 1).unwrap();
            m.send(0, 2, 66, FOREVER, 2).unwrap();
            assert_eq!(m.next_deadline(), Some(10));
            assert_eq!(get(&mut m, now), 0);
            let status = if now < 10 { 0 } else { -110 };
            assert_eq!(m.take(1), Some(Completion::new(status, 0)));
            for n in 1..4 {
                assert_eq!(get(&mut m, now), n);
            }
            if now < 10 {
                assert_eq!(get(&mut m, now), 55);
            }
            assert_eq!(get(&mut m, now), 66);
            assert_eq!(m.take(0), Some(Completion::new(0, 0)));
            assert_eq!(m.receive(2, 0, now), Err(-11));
            assert_eq!((m.occupancy(2), m.high_water(2)), (0, 4));
        }
    }

    #[test]
    fn direct_delivery_or_receiver_timeout() {
        for at in [6, 7] {
            let mut m = MailboxIpc::new([4, 0, 0]);
            m.receive(2, 7, 0).unwrap();
            assert_eq!(m.send(1, 2, 80, 0, 1), Err(-13));
            m.send(0, 2, 80, 0, at).unwrap();
            if at < 7 {
                assert_eq!(m.take(2), Some(Completion::new(0, 80)));
                assert_eq!(m.high_water(2), 0);
            } else {
                assert_eq!(m.take(2), Some(Completion::new(-110, 0)));
                assert_eq!(get(&mut m, at), 80);
            }
        }
    }

    #[test]
    fn sender_and_owner_death_cancel_only_uncommitted_words() {
        let mut m = MailboxIpc::new([4, 4, 0]);
        fill(&mut m);
        m.send(0, 2, 77, FOREVER, 1).unwrap();
        m.exit(0, 2);
        for n in 0..4 {
            assert_eq!(get(&mut m, 3), n);
        }
        assert_eq!(m.receive(2, 0, 3), Err(-11));
        for n in 0..4 {
            m.send(1, 2, n, 0, 4).unwrap();
            m.take(1);
        }
        m.send(1, 2, 99, FOREVER, 4).unwrap();
        m.exit(2, 5);
        assert_eq!(m.take(1), Some(Completion::new(-32, 0)));
        assert_eq!(m.send(1, 2, 13, 0, 6), Err(-32));
        assert_eq!((m.occupancy(2), m.high_water(2)), (0, 4));
    }
}
