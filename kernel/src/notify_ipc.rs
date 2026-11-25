//! Independent pulse FIFO and event latch per native endpoint. All operations
//! are serialized by the single-core runtime; waits use absolute monotonic ns.
const CAP: usize = 4;

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
enum Wait {
    None,
    Pulse(u64),
    Events { mask: u32, deadline: u64 },
}

impl Wait {
    fn deadline(self) -> Option<u64> {
        match self {
            Self::Pulse(d) | Self::Events { deadline: d, .. } if d != u64::MAX => Some(d),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Endpoint {
    alive: bool,
    pulses: [(u32, u64); CAP],
    head: usize,
    len: usize,
    peak: usize,
    bits: u32,
    wait: Wait,
    result: Option<Completion>,
}

impl Endpoint {
    const EMPTY: Self = Self {
        alive: true,
        pulses: [(0, 0); CAP],
        head: 0,
        len: 0,
        peak: 0,
        bits: 0,
        wait: Wait::None,
        result: None,
    };

    fn complete(&mut self, status: i32, x1: u64, x2: u64) {
        self.wait = Wait::None;
        self.result = Some(Completion::new(status, x1, x2));
    }

    fn consume(&mut self, mask: u32) -> bool {
        let matched = self.bits & mask;
        if matched == 0 {
            return false;
        }
        self.bits &= !matched;
        self.complete(0, u64::from(matched), 0);
        true
    }
}

pub struct NotifyIpc<const N: usize> {
    routes: [u8; N],
    endpoints: [Endpoint; N],
}

impl<const N: usize> NotifyIpc<N> {
    pub const fn new(routes: [u8; N]) -> Self {
        assert!(N >= 2 && N <= 3);
        Self {
            routes,
            endpoints: [Endpoint::EMPTY; N],
        }
    }

    fn available(&self, pid: usize) -> Result<(), i32> {
        let e = self.endpoints.get(pid).filter(|e| e.alive).ok_or(-3)?;
        if e.result.is_some() || !matches!(e.wait, Wait::None) {
            return Err(-16);
        }
        Ok(())
    }

    fn route(&self, sender: usize, owner: usize) -> Result<(), i32> {
        self.available(sender)?;
        if owner >= N {
            return Err(-22);
        }
        if self.routes[sender] & (1 << owner) == 0 {
            return Err(-13);
        }
        if !self.endpoints[owner].alive {
            return Err(-32);
        }
        Ok(())
    }

    fn mask(raw: u64) -> Result<u32, i32> {
        let bits = u32::try_from(raw).map_err(|_| -22)?;
        if bits == 0 {
            return Err(-22);
        }
        Ok(bits)
    }

    fn check_deadline(deadline: u64, now: u64) -> Result<(), i32> {
        if deadline != 0 && deadline != u64::MAX && deadline <= now {
            return Err(-110);
        }
        Ok(())
    }

    pub fn advance(&mut self, now: u64) {
        for e in &mut self.endpoints {
            if e.wait.deadline().is_some_and(|d| d <= now) {
                e.complete(-110, 0, 0);
            }
        }
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.endpoints
            .iter()
            .filter_map(|e| e.wait.deadline())
            .min()
    }

    pub fn take(&mut self, pid: usize) -> Option<Completion> {
        self.endpoints.get_mut(pid)?.result.take()
    }

    pub fn blocked(&self, pid: usize) -> bool {
        self.endpoints
            .get(pid)
            .is_some_and(|e| !matches!(e.wait, Wait::None))
    }

    pub fn occupancy(&self, owner: usize) -> usize {
        self.endpoints.get(owner).map_or(0, |e| e.len)
    }

    pub fn high_water(&self, owner: usize) -> usize {
        self.endpoints.get(owner).map_or(0, |e| e.peak)
    }

    pub fn pending_mask(&self, owner: usize) -> u32 {
        self.endpoints.get(owner).map_or(0, |e| e.bits)
    }

    pub fn pulse_send(
        &mut self,
        sender: usize,
        owner: usize,
        code: u64,
        word: u64,
        now: u64,
    ) -> Result<(), i32> {
        let code = u32::try_from(code).map_err(|_| -22)?;
        self.advance(now);
        self.route(sender, owner)?;
        let e = &mut self.endpoints[owner];
        if matches!(e.wait, Wait::Pulse(_)) {
            e.complete(0, u64::from(code), word);
        } else {
            if e.len == CAP {
                return Err(-11);
            }
            e.pulses[(e.head + e.len) % CAP] = (code, word);
            e.len += 1;
            e.peak = e.peak.max(e.len);
        }
        self.endpoints[sender].complete(0, 0, 0);
        Ok(())
    }

    pub fn pulse_recv(&mut self, owner: usize, deadline: u64, now: u64) -> Result<(), i32> {
        self.advance(now);
        self.available(owner)?;
        Self::check_deadline(deadline, now)?;
        let e = &mut self.endpoints[owner];
        if e.len == 0 {
            if deadline == 0 {
                return Err(-11);
            }
            e.wait = Wait::Pulse(deadline);
        } else {
            let (code, word) = e.pulses[e.head];
            e.pulses[e.head] = (0, 0);
            e.head = (e.head + 1) % CAP;
            e.len -= 1;
            e.complete(0, u64::from(code), word);
        }
        Ok(())
    }

    pub fn event_set(
        &mut self,
        sender: usize,
        owner: usize,
        mask: u64,
        now: u64,
    ) -> Result<(), i32> {
        let bits = Self::mask(mask)?;
        self.advance(now);
        self.route(sender, owner)?;
        let e = &mut self.endpoints[owner];
        e.bits |= bits;
        if let Wait::Events { mask, .. } = e.wait {
            e.consume(mask);
        }
        self.endpoints[sender].complete(0, 0, 0);
        Ok(())
    }

    pub fn event_take(
        &mut self,
        owner: usize,
        mask: u64,
        deadline: u64,
        now: u64,
    ) -> Result<(), i32> {
        let mask = Self::mask(mask)?;
        self.advance(now);
        self.available(owner)?;
        Self::check_deadline(deadline, now)?;
        let e = &mut self.endpoints[owner];
        if !e.consume(mask) {
            if deadline == 0 {
                return Err(-11);
            }
            e.wait = Wait::Events { mask, deadline };
        }
        Ok(())
    }

    pub fn exit(&mut self, pid: usize, now: u64) {
        self.advance(now);
        if let Some(e) = self.endpoints.get_mut(pid) {
            *e = Endpoint {
                alive: false,
                peak: e.peak,
                ..Endpoint::EMPTY
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(m: &mut NotifyIpc<3>, code: u64, word: u64, now: u64) {
        m.pulse_send(0, 2, code, word, now).unwrap();
        assert_eq!(m.take(0), Some(Completion::new(0, 0, 0)));
    }
    fn set(m: &mut NotifyIpc<3>, mask: u64, now: u64) {
        m.event_set(0, 2, mask, now).unwrap();
        assert_eq!(m.take(0), Some(Completion::new(0, 0, 0)));
    }

    #[test]
    fn capacity_order_wrap_width_and_owner_teardown() {
        let mut m = NotifyIpc::new([4, 0, 0]);
        assert_eq!(m.pending_mask(2), 0);
        assert_eq!(m.pulse_send(1, 2, 0, 0, 0), Err(-13));
        assert_eq!(m.pulse_send(0, 2, 1 << 32, 0, 0), Err(-22));
        for i in 0..4 {
            send(&mut m, i, u64::MAX - i, 0);
        }
        assert_eq!(m.pulse_send(0, 2, 9, 11, 0), Err(-11));
        m.pulse_recv(2, 0, 0).unwrap();
        assert_eq!(m.take(2), Some(Completion::new(0, 0, u64::MAX)));
        send(&mut m, 4, u64::MAX - 4, 0);
        set(&mut m, 7, 0);
        m.exit(0, 0);
        for i in 1..5 {
            m.pulse_recv(2, 0, 0).unwrap();
            assert_eq!(m.take(2), Some(Completion::new(0, i, u64::MAX - i)));
        }
        assert_eq!(m.pending_mask(2), 7);
        m.exit(2, 1);
        assert_eq!(
            (m.occupancy(2), m.high_water(2), m.pending_mask(2)),
            (0, 4, 0)
        );
    }

    #[test]
    fn direct_pulse_or_equal_deadline_leaves_pulse_queued() {
        for at in [7, 8, 9] {
            let mut m = NotifyIpc::new([4, 0, 0]);
            m.pulse_recv(2, 8, 0).unwrap();
            set(&mut m, 1, 1);
            assert_eq!(m.next_deadline(), Some(8));
            send(&mut m, 99, 55, at);
            let pulse = Some(Completion::new(0, 99, 55));
            if at < 8 {
                assert_eq!(m.take(2), pulse);
                assert_eq!(m.high_water(2), 0);
            } else {
                assert_eq!(m.take(2), Some(Completion::new(-110, 0, 0)));
                m.pulse_recv(2, 0, at).unwrap();
                assert_eq!(m.take(2), pulse);
            }
            assert_eq!(m.pending_mask(2), 1);
        }
    }

    #[test]
    fn selective_wait_coalescing_and_pulse_independence() {
        let mut m = NotifyIpc::new([4, 0, 0]);
        for invalid in [0, 1 << 32] {
            assert_eq!(m.event_set(0, 2, invalid, 0), Err(-22));
            assert_eq!(m.event_take(2, invalid, 0, 0), Err(-22));
        }
        m.event_take(2, 3, u64::MAX, 1).unwrap();
        send(&mut m, 4, 8, 2);
        set(&mut m, 4, 2);
        set(&mut m, 4, 2);
        assert!(m.blocked(2));
        assert_eq!(m.next_deadline(), None);
        set(&mut m, 3, 3);
        assert_eq!(m.take(2), Some(Completion::new(0, 3, 0)));
        assert_eq!((m.pending_mask(2), m.occupancy(2)), (4, 1));
        m.event_take(2, 4, 0, 3).unwrap();
        assert_eq!(m.take(2), Some(Completion::new(0, 4, 0)));
        assert_eq!(m.event_take(2, 4, 0, 3), Err(-11));
        m.event_take(2, 1, 4, 3).unwrap();
        set(&mut m, 1, 4);
        assert_eq!(m.take(2), Some(Completion::new(-110, 0, 0)));
        assert_eq!(m.pending_mask(2), 1);
        m.exit(2, 5);
        assert_eq!(m.pulse_send(0, 2, 1, 1, 5), Err(-32));
        assert_eq!(m.event_set(0, 2, 1, 5), Err(-32));
    }
}
