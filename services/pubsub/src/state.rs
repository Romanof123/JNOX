//! Four static topics in one serialized EL0 broker. Client IDs 0..2 are
//! supplied by the authenticated transport. Grant arrays are indexed by PID.
//! Each publication is committed in registration order after a full preflight.
pub const TOPICS: usize = 4;
pub const CLIENTS: usize = 3;
pub const PER_CLIENT: usize = 3;
pub const DEPTH: usize = 4;
pub const PAYLOAD: usize = 32;
const SLOTS: usize = CLIENTS * PER_CLIENT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub slot: u8,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub sequence: u64,
    pub len: u8,
    pub data: [u8; PAYLOAD],
}

impl Record {
    const EMPTY: Self = Self {
        sequence: 0,
        len: 0,
        data: [0; PAYLOAD],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub queued: u8,
    pub high_water: u8,
    pub lost: u64,
    pub front_sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub active: usize,
    pub queued: usize,
    pub peak_active: usize,
    pub peak_queued: usize,
    pub last_sequence: [u64; TOPICS],
    /// Lifetime drops on full subscriber queues, including retired subscriptions.
    pub dropped: u64,
}

#[derive(Clone, Copy)]
struct Subscription {
    generation: u64,
    topic: Option<u8>,
    records: [Record; DEPTH],
    head: usize,
    len: usize,
    high_water: u8,
    lost: u64,
}

impl Subscription {
    const EMPTY: Self = Self {
        generation: 0,
        topic: None,
        records: [Record::EMPTY; DEPTH],
        head: 0,
        len: 0,
        high_water: 0,
        lost: 0,
    };
}

pub struct PubSub {
    publish_grants: [u8; 4],
    subscribe_grants: [u8; 4],
    may_configure: bool,
    alive: [bool; CLIENTS],
    slots: [Subscription; SLOTS],
    /// Active IDs are compacted on removal. Reuse appends at the tail.
    order: [u8; SLOTS],
    active: usize,
    peak_active: usize,
    peak_queued: usize,
    last_sequence: [u64; TOPICS],
    dropped: u64,
}

impl Clone for PubSub {
    fn clone(&self) -> Self {
        let mut copy = Self::new(self.publish_grants, self.subscribe_grants);
        copy.copy_from(self);
        copy
    }

    fn clone_from(&mut self, source: &Self) {
        self.copy_from(source);
    }
}

impl PubSub {
    pub const fn new(publish: [u8; 4], subscribe: [u8; 4]) -> Self {
        Self {
            publish_grants: publish,
            subscribe_grants: subscribe,
            may_configure: false,
            alive: [true; CLIENTS],
            slots: [Subscription::EMPTY; SLOTS],
            order: [0; SLOTS],
            active: 0,
            peak_active: 0,
            peak_queued: 0,
            last_sequence: [0; TOPICS],
            dropped: 0,
        }
    }

    /// Use for static storage when grants come from the package at startup.
    pub const fn unconfigured() -> Self {
        let mut state = Self::new([0; 4], [0; 4]);
        state.may_configure = true;
        state
    }

    pub fn configure_grants(&mut self, publish: [u8; 4], subscribe: [u8; 4]) -> Result<(), i32> {
        if !self.may_configure {
            return Err(-16);
        }
        if publish[3] != 0
            || subscribe[3] != 0
            || publish.iter().chain(&subscribe).any(|&mask| mask & !0x0f != 0)
        {
            return Err(-22);
        }
        self.publish_grants = publish;
        self.subscribe_grants = subscribe;
        self.may_configure = false;
        Ok(())
    }

    /// Prepare the broker's preallocated transaction buffer without returning
    /// all queues by value. The broker switches buffers only on a valid reply.
    pub fn copy_from(&mut self, source: &Self) {
        self.publish_grants = source.publish_grants;
        self.subscribe_grants = source.subscribe_grants;
        self.may_configure = source.may_configure;
        self.alive = source.alive;
        self.slots.copy_from_slice(&source.slots);
        self.order = source.order;
        self.active = source.active;
        self.peak_active = source.peak_active;
        self.peak_queued = source.peak_queued;
        self.last_sequence = source.last_sequence;
        self.dropped = source.dropped;
    }

    fn client(&self, pid: usize) -> Result<(), i32> {
        if pid >= CLIENTS {
            return Err(-22);
        }
        if !self.alive[pid] {
            return Err(-3);
        }
        Ok(())
    }

    fn grant(&self, pid: usize, topic: usize, grants: &[u8; 4]) -> Result<(), i32> {
        self.client(pid)?;
        if topic >= TOPICS {
            return Err(-22);
        }
        if grants[pid] & (1 << topic) == 0 {
            return Err(-13);
        }
        Ok(())
    }

    fn owned(&self, pid: usize, h: Handle) -> Result<usize, i32> {
        self.client(pid)?;
        let index = usize::from(h.slot);
        let slot = self.slots.get(index).ok_or(-22)?;
        if index / PER_CLIENT != pid {
            return Err(-1);
        }
        if slot.topic.is_none() || slot.generation != h.generation {
            return Err(-116);
        }
        Ok(index)
    }

    pub fn subscribe(&mut self, pid: usize, topic: usize) -> Result<Handle, i32> {
        self.grant(pid, topic, &self.subscribe_grants)?;
        let range = pid * PER_CLIENT..(pid + 1) * PER_CLIENT;
        let mut error = -28;
        for index in range {
            let slot = &mut self.slots[index];
            if slot.topic.is_some() {
                continue;
            }
            let Some(next) = slot.generation.checked_add(1) else {
                // This free slot is permanently retired. Try the other two.
                error = -75;
                continue;
            };
            slot.generation = next;
            slot.topic = Some(topic as u8);
            self.order[self.active] = index as u8;
            self.active += 1;
            self.peak_active = self.peak_active.max(self.active);
            return Ok(Handle {
                slot: index as u8,
                generation: next,
            });
        }
        Err(error)
    }

    pub fn publish(&mut self, pid: usize, topic: usize, payload: &[u8]) -> Result<u64, i32> {
        self.grant(pid, topic, &self.publish_grants)?;
        if payload.len() > PAYLOAD {
            return Err(-90);
        }
        let sequence = self.last_sequence[topic].checked_add(1).ok_or(-75)?;
        let target = Some(topic as u8);
        let mut drops = 0;
        for &id in &self.order[..self.active] {
            let slot = &self.slots[usize::from(id)];
            if slot.topic == target && slot.len == DEPTH {
                slot.lost.checked_add(1).ok_or(-75)?;
                drops += 1;
            }
        }
        let total_drops = self.dropped.checked_add(drops).ok_or(-75)?;

        let mut record = Record {
            sequence,
            len: payload.len() as u8,
            ..Record::EMPTY
        };
        record.data[..payload.len()].copy_from_slice(payload);
        for &id in &self.order[..self.active] {
            let slot = &mut self.slots[usize::from(id)];
            if slot.topic != target {
                continue;
            }
            if slot.len == DEPTH {
                slot.lost += 1;
            } else {
                slot.records[(slot.head + slot.len) % DEPTH] = record;
                slot.len += 1;
                slot.high_water = slot.high_water.max(slot.len as u8);
            }
        }
        self.last_sequence[topic] = sequence;
        self.dropped = total_drops;
        self.peak_queued = self.peak_queued.max(self.queued());
        Ok(sequence)
    }

    pub fn observe(&self, pid: usize, h: Handle) -> Result<Observation, i32> {
        let slot = &self.slots[self.owned(pid, h)?];
        Ok(Observation {
            queued: slot.len as u8,
            high_water: slot.high_water,
            lost: slot.lost,
            front_sequence: (slot.len != 0).then(|| slot.records[slot.head].sequence),
        })
    }

    pub fn peek(&self, pid: usize, h: Handle) -> Result<Record, i32> {
        let slot = &self.slots[self.owned(pid, h)?];
        if slot.len == 0 {
            return Err(-11);
        }
        Ok(slot.records[slot.head])
    }

    /// A waiting broker can report terminal EPIPE after the retained queue
    /// drains and the last authorized publisher exits. Nonblocking I/O uses EAGAIN.
    pub fn has_live_publisher(&self, pid: usize, h: Handle) -> Result<bool, i32> {
        let topic = self.slots[self.owned(pid, h)?].topic.unwrap();
        Ok(self
            .alive
            .iter()
            .enumerate()
            .any(|(id, &live)| live && self.publish_grants[id] & (1 << topic) != 0))
    }

    pub fn receive(&mut self, pid: usize, h: Handle) -> Result<Record, i32> {
        let index = self.owned(pid, h)?;
        let slot = &mut self.slots[index];
        if slot.len == 0 {
            return Err(-11);
        }
        let record = slot.records[slot.head];
        slot.records[slot.head] = Record::EMPTY;
        slot.head = (slot.head + 1) % DEPTH;
        slot.len -= 1;
        Ok(record)
    }

    fn remove(&mut self, id: usize) {
        let position = self.order[..self.active]
            .iter()
            .position(|&v| usize::from(v) == id)
            .unwrap();
        self.order.copy_within(position + 1..self.active, position);
        self.active -= 1;
        self.order[self.active] = 0;
        self.slots[id] = Subscription {
            generation: self.slots[id].generation,
            ..Subscription::EMPTY
        };
    }

    pub fn unsubscribe(&mut self, pid: usize, h: Handle) -> Result<(), i32> {
        let index = self.owned(pid, h)?;
        self.remove(index);
        Ok(())
    }

    pub fn exit(&mut self, pid: usize) {
        if self.client(pid).is_err() {
            return;
        }
        self.alive[pid] = false;
        self.may_configure = false;
        for index in pid * PER_CLIENT..(pid + 1) * PER_CLIENT {
            if self.slots[index].topic.is_some() {
                self.remove(index);
            }
        }
    }

    fn queued(&self) -> usize {
        self.slots.iter().map(|slot| slot.len).sum()
    }

    pub fn stats(&self) -> Stats {
        Stats {
            active: self.active,
            queued: self.queued(),
            peak_active: self.peak_active,
            peak_queued: self.peak_queued,
            last_sequence: self.last_sequence,
            dropped: self.dropped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: u8) -> [u8; 32] {
        core::array::from_fn(|i| id.wrapping_add(i as u8))
    }

    #[test]
    fn stalled_subscriber_loses_only_its_newest_and_can_resume() {
        let mut s = PubSub::new([1, 0, 0, 0], [0, 1, 1, 0]);
        let slow = s.subscribe(2, 0).unwrap();
        let fast = s.subscribe(1, 0).unwrap();
        for id in 1..=6u8 {
            assert_eq!(s.publish(0, 0, &sample(id)), Ok(u64::from(id)));
            let rec = s.receive(1, fast).unwrap();
            assert_eq!((rec.sequence, rec.len, rec.data), (u64::from(id), 32, sample(id)));
        }
        assert_eq!(
            s.observe(1, fast).unwrap(),
            Observation {
                queued: 0,
                high_water: 1,
                lost: 0,
                front_sequence: None
            }
        );
        let observation = Observation {
            queued: 4,
            high_water: 4,
            lost: 2,
            front_sequence: Some(1),
        };
        assert_eq!(s.observe(2, slow), Ok(observation));
        let peeked = s.peek(2, slow).unwrap();
        assert_eq!((peeked.sequence, peeked.data), (1, sample(1)));
        assert_eq!(s.observe(2, slow), Ok(observation));
        assert_eq!(s.receive(2, slow), Ok(peeked));
        assert_eq!(s.publish(0, 0, &sample(7)), Ok(7));
        for id in [2, 3, 4, 7] {
            let rec = s.receive(2, slow).unwrap();
            assert_eq!((rec.sequence, rec.data), (u64::from(id), sample(id)));
        }
        assert_eq!(s.receive(2, slow), Err(-11));
        assert_eq!(
            (s.stats().peak_active, s.stats().peak_queued, s.stats().dropped),
            (2, 5, 2)
        );
        s.exit(1);
        s.exit(2);
        assert_eq!((s.stats().active, s.stats().queued, s.stats().dropped), (0, 0, 2));
    }

    #[test]
    fn grants_are_directional_and_quota_is_per_client() {
        let mut s = PubSub::new([1, 2, 0, 15], [0, 1, 2, 15]);
        assert_eq!(s.subscribe(0, 0), Err(-13));
        assert_eq!(s.publish(1, 0, b"x"), Err(-13));
        assert_eq!(s.subscribe(3, 0), Err(-22));
        assert_eq!(s.publish(0, 4, b"x"), Err(-22));
        let a = s.subscribe(1, 0).unwrap();
        let b = s.subscribe(1, 0).unwrap();
        let c = s.subscribe(1, 0).unwrap();
        assert_eq!(s.subscribe(1, 0), Err(-28));
        let d = s.subscribe(2, 1).unwrap();
        assert_eq!(s.publish(0, 0, &[9; 33]), Err(-90));
        assert_eq!(s.stats().last_sequence, [0; 4]);
        assert_eq!(s.observe(2, a), Err(-1));
        assert_eq!(s.receive(2, b), Err(-1));
        assert_eq!(s.unsubscribe(2, c), Err(-1));
        s.publish(1, 1, &[]).unwrap();
        let zero = s.receive(2, d).unwrap();
        assert_eq!((zero.sequence, zero.len, zero.data), (1, 0, [0; 32]));
        assert_eq!(s.receive(1, a), Err(-11));
        s.publish(0, 0, b"x").unwrap();
        assert_eq!(s.stats().last_sequence, [1, 1, 0, 0]);
        assert_eq!(s.stats().queued, 3);
    }

    #[test]
    fn unregister_and_death_preserve_other_queues_and_registration_order() {
        let mut s = PubSub::new([1, 0, 0, 0], [0, 1, 1, 0]);
        let a = s.subscribe(1, 0).unwrap();
        let b = s.subscribe(2, 0).unwrap();
        let c = s.subscribe(1, 0).unwrap();
        s.publish(0, 0, b"old").unwrap();
        s.unsubscribe(1, a).unwrap();
        let fresh = s.subscribe(1, 0).unwrap();
        assert_eq!(fresh.slot, a.slot);
        assert!(fresh.generation > a.generation);
        assert_eq!(&s.order[..s.active], &[b.slot, c.slot, fresh.slot]);
        assert_eq!(s.observe(1, a), Err(-116));
        assert_eq!(s.unsubscribe(1, a), Err(-116));
        assert_eq!(s.observe(1, fresh).unwrap().high_water, 0);
        assert_eq!(s.receive(1, fresh), Err(-11));
        s.exit(2);
        s.exit(2);
        assert_eq!(&s.order[..s.active], &[c.slot, fresh.slot]);
        assert_eq!(s.subscribe(2, 0), Err(-3));
        assert_eq!(s.receive(1, c).unwrap().sequence, 1);
        assert_eq!(s.publish(0, 0, b"new"), Ok(2));
        assert_eq!(s.receive(1, fresh).unwrap().sequence, 2);
        assert_eq!(s.receive(1, c).unwrap().sequence, 2);
    }

    #[test]
    fn expired_reply_discards_staged_subscribe_publish_and_receive() {
        let mut live = PubSub::new([1, 0, 0, 0], [0, 1, 0, 0]);
        let mut staged = PubSub::new([0; 4], [0; 4]);
        staged.copy_from(&live);
        let unused = staged.subscribe(1, 0).unwrap();
        assert_eq!(live.stats().active, 0);
        assert_eq!(live.peek(1, unused), Err(-116));
        let h = live.subscribe(1, 0).unwrap();
        staged.clone_from(&live);
        staged.publish(0, 0, b"cancelled").unwrap();
        assert_eq!(live.stats().last_sequence, [0; 4]);
        assert_eq!(live.peek(1, h), Err(-11));
        staged.copy_from(&live);
        assert_eq!(staged.publish(0, 0, b"committed"), Ok(1));
        core::mem::swap(&mut live, &mut staged);
        staged.copy_from(&live);
        assert_eq!(staged.receive(1, h).unwrap().sequence, 1);
        assert_eq!(live.receive(1, h).unwrap().sequence, 1);
        assert_eq!(live.receive(1, h), Err(-11));
    }

    #[test]
    fn counter_edges_reject_before_any_subscriber_changes() {
        let mut s = PubSub::new([1, 0, 0, 0], [0, 1, 1, 0]);
        let fast = s.subscribe(1, 0).unwrap();
        let full = s.subscribe(2, 0).unwrap();
        for _ in 0..DEPTH {
            s.publish(0, 0, b"fill").unwrap();
            s.receive(1, fast).unwrap();
        }
        for individual in [true, false] {
            s.slots[usize::from(full.slot)].lost = if individual { u64::MAX } else { 0 };
            s.dropped = if individual { 0 } else { u64::MAX };
            let before = s.stats();
            let full_before = s.observe(2, full).unwrap();
            assert_eq!(s.publish(0, 0, b"must not reach fast"), Err(-75));
            assert_eq!(s.stats(), before);
            assert_eq!(s.observe(2, full).unwrap(), full_before);
            assert_eq!(s.peek(1, fast), Err(-11));
        }
        s.unsubscribe(2, full).unwrap();
        s.last_sequence[0] = u64::MAX - 1;
        assert_eq!(s.publish(0, 0, b"last"), Ok(u64::MAX));
        assert_eq!(s.publish(0, 0, b"overflow"), Err(-75));
        assert_eq!(s.receive(1, fast).unwrap().sequence, u64::MAX);
        assert_eq!(s.receive(1, fast), Err(-11));
    }

    #[test]
    fn retired_slot_generations_never_regrant_an_old_handle() {
        let mut s = PubSub::new([0; 4], [0, 1, 0, 0]);
        for local in 0..PER_CLIENT {
            s.slots[PER_CLIENT + local].generation = u64::MAX - 1;
        }
        for local in 0..PER_CLIENT {
            let h = s.subscribe(1, 0).unwrap();
            assert_eq!(
                (usize::from(h.slot), h.generation),
                (PER_CLIENT + local, u64::MAX)
            );
            s.unsubscribe(1, h).unwrap();
            assert_eq!(s.observe(1, h), Err(-116));
        }
        let before = s.stats();
        assert_eq!(s.subscribe(1, 0), Err(-75));
        assert_eq!(s.stats(), before);
    }

    #[test]
    fn final_publisher_exit_leaves_buffered_records_for_drain() {
        let mut s = PubSub::new([1, 1, 2, 0], [0, 0, 1, 0]);
        let h = s.subscribe(2, 0).unwrap();
        s.publish(0, 0, b"retained").unwrap();
        s.exit(0);
        assert_eq!(s.has_live_publisher(2, h), Ok(true));
        s.exit(1);
        assert_eq!(s.has_live_publisher(2, h), Ok(false));
        assert_eq!(s.receive(2, h).unwrap().sequence, 1);
        assert_eq!(s.peek(2, h), Err(-11));
        assert_eq!(s.receive(2, h), Err(-11));
        s.unsubscribe(2, h).unwrap();
        assert_eq!(s.has_live_publisher(2, h), Err(-116));
    }

    #[test]
    fn package_grants_are_a_one_shot_startup_configuration() {
        let mut s = PubSub::unconfigured();
        assert_eq!(s.publish(0, 0, b"x"), Err(-13));
        assert_eq!(s.configure_grants([1, 0, 0, 1], [0; 4]), Err(-22));
        assert_eq!(s.configure_grants([1, 0, 0, 0], [0, 1, 0, 0]), Ok(()));
        assert_eq!(s.configure_grants([15; 4], [15; 4]), Err(-16));
        assert_eq!(s.publish(0, 0, b"x"), Ok(1));
    }
}
