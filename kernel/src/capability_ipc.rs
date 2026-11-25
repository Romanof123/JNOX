//! CPU-serialized authority for the existing mailbox transport. Handles are
//! scoped to the issuing process. Target references live in occupied entries,
//! including transfers awaiting receipt. Notices themselves hold no extra ref.
pub const SEND: u8 = 1;
pub const DELEGATE: u8 = 2;
pub const HANDLES: usize = 4;
pub const NOTICES: usize = 2;
const LAST_GENERATION: u64 = u64::MAX >> 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    pub handle: u64,
    pub tag: u64,
    pub sender: usize,
    pub target: usize,
    pub rights: u8,
    pub moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub alive: bool,
    pub handles: usize,
    pub peak_handles: usize,
    pub notices: usize,
    pub peak_notices: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    pub generation: u64,
    pub target: Option<usize>,
    pub rights: u8,
    pub delivered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Grant {
    target: usize,
    rights: u8,
    delivered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    generation: u64,
    grant: Option<Grant>,
    // Revocation releases the reference immediately. EPIPE identifies the
    // most recent dead-target generation until the slot is allocated again.
    revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Process {
    alive: bool,
    entries: [Entry; HANDLES],
    origins: [u64; HANDLES],
    origin_count: usize,
    notices: [Option<Notice>; NOTICES],
    notice_count: usize,
    peak_handles: usize,
    peak_notices: usize,
}

impl Process {
    const NEW: Self = Self {
        alive: true,
        entries: [Entry {
            generation: 0,
            grant: None,
            revoked: false,
        }; HANDLES],
        origins: [0; HANDLES],
        origin_count: 0,
        notices: [None; NOTICES],
        notice_count: 0,
        peak_handles: 0,
        peak_notices: 0,
    };

    fn handles(&self) -> usize {
        self.entries.iter().filter(|e| e.grant.is_some()).count()
    }

    fn vacant(&self) -> Result<usize, i32> {
        let mut error = -24;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.grant.is_none() {
                if entry.generation < LAST_GENERATION {
                    return Ok(i);
                }
                error = -75;
            }
        }
        Err(error)
    }

    fn install(&mut self, owner: usize, slot: usize, grant: Grant) -> u64 {
        let generation = self.entries[slot].generation + 1; // checked by vacant()
        self.entries[slot] = Entry {
            generation,
            grant: Some(grant),
            revoked: false,
        };
        self.peak_handles = self.peak_handles.max(self.handles());
        (generation << 4) | ((owner as u64) << 2) | slot as u64
    }

    fn remove_notices_for(&mut self, dead: usize) {
        let mut keep = 0;
        for i in 0..self.notice_count {
            let n = self.notices[i].take().unwrap();
            if n.target != dead {
                self.notices[keep] = Some(n);
                keep += 1;
            }
        }
        self.notice_count = keep;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityIpc<const N: usize> {
    transfer_routes: [u8; N],
    processes: [Process; N],
}

impl<const N: usize> CapabilityIpc<N> {
    pub const fn new(transfer_routes: [u8; N]) -> Self {
        assert!(N >= 2 && N <= 4);
        Self {
            transfer_routes,
            processes: [Process::NEW; N],
        }
    }

    fn owner(&self, pid: usize) -> Result<&Process, i32> {
        let p = self.processes.get(pid).ok_or(-22)?;
        if !p.alive {
            return Err(-3);
        }
        Ok(p)
    }

    fn peer(&self, pid: usize) -> Result<&Process, i32> {
        let p = self.processes.get(pid).ok_or(-22)?;
        if !p.alive {
            return Err(-32);
        }
        Ok(p)
    }

    fn valid_rights(rights: u8) -> Result<(), i32> {
        if rights == 0 || rights & !(SEND | DELEGATE) != 0 {
            Err(-22)
        } else {
            Ok(())
        }
    }

    fn owned(&self, owner: usize, handle: u64) -> Result<(usize, Grant), i32> {
        let p = self.owner(owner)?;
        let generation = handle >> 4;
        if generation == 0 {
            return Err(-116);
        }
        if ((handle >> 2) & 3) as usize != owner {
            return Err(-1);
        }
        let slot = (handle & 3) as usize;
        let entry = p.entries[slot];
        if entry.generation != generation {
            return Err(-116);
        }
        if entry.revoked {
            return Err(-32);
        }
        let grant = entry.grant.filter(|g| g.delivered).ok_or(-116)?;
        Ok((slot, grant))
    }

    /// Trusted package installation only. SVC lookup reads this origin's
    /// original identity, and never synthesizes a replacement from a slot.
    pub fn grant(&mut self, owner: usize, target: usize, rights: u8) -> Result<u64, i32> {
        let p = self.owner(owner)?;
        self.peer(target)?;
        Self::valid_rights(rights)?;
        if p.origin_count == HANDLES {
            return Err(-24);
        }
        let slot = p.vacant()?;
        let p = &mut self.processes[owner];
        let handle = p.install(
            owner,
            slot,
            Grant {
                target,
                rights,
                delivered: true,
            },
        );
        p.origins[p.origin_count] = handle;
        p.origin_count += 1;
        Ok(handle)
    }

    pub fn initial(&self, owner: usize, ordinal: usize) -> Result<u64, i32> {
        let p = self.owner(owner)?;
        let handle = *p.origins[..p.origin_count].get(ordinal).ok_or(-22)?;
        self.owned(owner, handle)?;
        Ok(handle)
    }

    /// The caller uses this validated target in the ordinary bounded mailbox
    /// send path, under the same masked boundary as this resolution.
    pub fn resolve_send(&self, owner: usize, handle: u64) -> Result<usize, i32> {
        let (_, grant) = self.owned(owner, handle)?;
        if grant.rights & SEND == 0 {
            return Err(-13);
        }
        self.peer(grant.target)?;
        Ok(grant.target)
    }

    pub fn transfer(
        &mut self,
        owner: usize,
        handle: u64,
        recipient: usize,
        rights: u8,
        move_handle: bool,
        tag: u64,
    ) -> Result<(), i32> {
        let (source_slot, source) = self.owned(owner, handle)?;
        Self::valid_rights(rights)?;
        if recipient >= N || owner == recipient {
            return Err(-22);
        }
        if self.transfer_routes[owner] & (1 << recipient) == 0
            || source.rights & DELEGATE == 0
            || rights & !source.rights != 0
        {
            return Err(-13);
        }
        self.peer(source.target)?;
        let receiver = self.peer(recipient)?;
        let dest = receiver.vacant()?;
        if receiver.notice_count == NOTICES {
            return Err(-11);
        }

        let receiver = &mut self.processes[recipient];
        let installed = receiver.install(
            recipient,
            dest,
            Grant {
                target: source.target,
                rights,
                delivered: false,
            },
        );
        let notice = Notice {
            handle: installed,
            tag,
            sender: owner,
            target: source.target,
            rights,
            moved: move_handle,
        };
        receiver.notices[receiver.notice_count] = Some(notice);
        receiver.notice_count += 1;
        receiver.peak_notices = receiver.peak_notices.max(receiver.notice_count);
        if move_handle {
            self.processes[owner].entries[source_slot].grant = None;
        }
        Ok(())
    }

    pub fn receive(&mut self, owner: usize) -> Result<Notice, i32> {
        self.owner(owner)?;
        let p = &mut self.processes[owner];
        if p.notice_count == 0 {
            return Err(-11);
        }
        let notice = p.notices[0].take().unwrap();
        p.notices.copy_within(1..p.notice_count, 0);
        p.notice_count -= 1;
        p.notices[p.notice_count] = None;
        let entry = &mut p.entries[(notice.handle & 3) as usize];
        debug_assert_eq!(entry.generation, notice.handle >> 4);
        entry.grant.as_mut().unwrap().delivered = true;
        Ok(notice)
    }

    pub fn close(&mut self, owner: usize, handle: u64) -> Result<(), i32> {
        let (slot, _) = self.owned(owner, handle)?;
        self.processes[owner].entries[slot].grant = None;
        Ok(())
    }

    /// Mailbox teardown and handle teardown share the same CPU0 exit boundary.
    /// A committed transfer to surviving processes survives its sender's exit.
    pub fn exit(&mut self, dead: usize) {
        if self.owner(dead).is_err() {
            return;
        }
        self.processes[dead].alive = false;
        for (owner, p) in self.processes.iter_mut().enumerate() {
            for entry in &mut p.entries {
                if owner == dead {
                    entry.grant = None;
                } else if entry.grant.is_some_and(|g| g.target == dead) {
                    entry.grant = None;
                    entry.revoked = true;
                }
            }
            if owner == dead {
                p.notices = [None; NOTICES];
                p.notice_count = 0;
            } else {
                p.remove_notices_for(dead);
            }
        }
    }

    pub fn stats(&self, pid: usize) -> Option<Stats> {
        let p = self.processes.get(pid)?;
        Some(Stats {
            alive: p.alive,
            handles: p.handles(),
            peak_handles: p.peak_handles,
            notices: p.notice_count,
            peak_notices: p.peak_notices,
        })
    }

    pub fn slot(&self, owner: usize, slot: usize) -> Option<SlotInfo> {
        let entry = self.processes.get(owner)?.entries.get(slot)?;
        Some(SlotInfo {
            generation: entry.generation,
            target: entry.grant.map(|g| g.target),
            rights: entry.grant.map_or(0, |g| g.rights),
            delivered: entry.grant.is_some_and(|g| g.delivered),
        })
    }

    pub fn refs(&self, target: usize) -> usize {
        self.processes
            .iter()
            .flat_map(|p| &p.entries)
            .filter(|e| e.grant.is_some_and(|g| g.target == target))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTH: u8 = SEND | DELEGATE;

    #[test]
    fn copy_move_receipt_and_owner_scoped_attenuation() {
        let mut c = CapabilityIpc::new([2, 1, 0]);
        let source = c.grant(0, 2, BOTH).unwrap();
        assert_eq!(source, 16);
        assert_eq!(c.initial(0, 0), Ok(source));
        c.transfer(0, source, 1, SEND, false, 0xfecd_beef).unwrap();
        let before = c.clone();
        assert_eq!(c.resolve_send(1, 20), Err(-116)); // reserved, not delivered
        assert_eq!(c.close(1, 20), Err(-116));
        assert_eq!(c.resolve_send(1, source), Err(-1));
        assert_eq!(c, before);
        let copy = c.receive(1).unwrap();
        assert_eq!(
            (
                copy.handle,
                copy.tag,
                copy.sender,
                copy.target,
                copy.rights,
                copy.moved
            ),
            (20, 0xfecd_beef, 0, 2, SEND, false)
        );
        assert_eq!(c.resolve_send(0, source), Ok(2));
        assert_eq!(c.resolve_send(1, copy.handle), Ok(2));
        assert_eq!(c.receive(1), Err(-11));
        let before = c.clone();
        assert_eq!(c.transfer(1, copy.handle, 0, SEND, true, 3), Err(-13)); // cannot delegate
        assert_eq!(c.transfer(0, source, 2, SEND, true, 3), Err(-13)); // forbidden direction
        assert_eq!(c.transfer(0, source, 0, SEND, true, 3), Err(-22)); // self transfer
        assert_eq!(c.transfer(0, source, 1, 4, true, 3), Err(-22));
        assert_eq!(c, before);
        c.transfer(0, source, 1, DELEGATE, true, 45).unwrap();
        assert_eq!(c.resolve_send(0, source), Err(-116));
        assert_eq!(c.initial(0, 0), Err(-116));
        let moved = c.receive(1).unwrap();
        assert!(moved.moved);
        assert_eq!(c.resolve_send(1, moved.handle), Err(-13));
        assert_eq!(c.transfer(1, moved.handle, 0, BOTH, true, 9), Err(-13)); // no amplification
        c.transfer(1, moved.handle, 0, DELEGATE, true, 9).unwrap();
        let returned = c.receive(0).unwrap();
        assert_ne!(returned.handle, source);
        assert_eq!(c.initial(0, 0), Err(-116)); // original ordinal never confers the replacement
        assert_eq!(c.refs(2), 2);
        c.exit(0);
        c.exit(1);
        assert_eq!(c.refs(2), 0);
    }

    #[test]
    fn notice_pressure_and_handle_pressure_each_preserve_the_entire_move() {
        let mut c = CapabilityIpc::new([2, 0, 0]);
        let source = c.grant(0, 2, BOTH).unwrap();
        for tag in [17, 18] {
            c.transfer(0, source, 1, SEND, false, tag).unwrap();
        }
        let before = c.clone();
        assert_eq!(c.transfer(0, source, 1, SEND, true, 19), Err(-11));
        assert_eq!(c, before);
        let first = c.receive(1).unwrap();
        let second = c.receive(1).unwrap();
        assert_eq!([first.tag, second.tag], [17, 18]);
        for tag in [20, 21] {
            c.transfer(0, source, 1, SEND, false, tag).unwrap();
            assert_eq!(c.receive(1).unwrap().tag, tag);
        }
        let before = c.clone();
        assert_eq!(
            c.stats(1),
            Some(Stats {
                alive: true,
                handles: 4,
                peak_handles: 4,
                notices: 0,
                peak_notices: 2
            })
        );
        assert_eq!(c.transfer(0, source, 1, SEND, true, 22), Err(-24));
        assert_eq!(c, before);
        c.close(1, first.handle).unwrap();
        c.transfer(0, source, 1, SEND, true, 22).unwrap();
        let new = c.receive(1).unwrap();
        assert_eq!(new.tag, 22);
        assert_eq!(new.handle & 3, first.handle & 3);
        assert!(new.handle >> 4 > first.handle >> 4);
        assert_eq!(c.close(1, first.handle), Err(-116));
        assert_eq!(c.resolve_send(1, new.handle), Ok(2));
        assert_eq!(c.resolve_send(0, source), Err(-116));
        assert_eq!(c.refs(2), 4);
    }

    #[test]
    fn death_before_or_after_move_has_one_committed_owner() {
        for take_notice in [false, true] {
            let mut c = CapabilityIpc::new([2, 0, 0]);
            let source = c.grant(0, 2, BOTH).unwrap();
            let mut before = c.clone();
            before.exit(1);
            let dead = before.clone();
            assert_eq!(before.transfer(0, source, 1, SEND, true, 9), Err(-32));
            assert_eq!(before, dead);
            assert_eq!(before.resolve_send(0, source), Ok(2));
            c.transfer(0, source, 1, SEND, true, 9).unwrap();
            if take_notice {
                c.receive(1).unwrap();
            }
            c.exit(1);
            c.exit(1);
            assert_eq!((c.refs(2), c.stats(1).unwrap().notices), (0, 0));
            assert_eq!(c.resolve_send(0, source), Err(-116));
        }
    }

    #[test]
    fn target_teardown_removes_only_its_references_and_notices() {
        let mut c = CapabilityIpc::new([2, 0, 0, 0]);
        let dying = c.grant(0, 2, BOTH).unwrap();
        let healthy = c.grant(0, 3, BOTH).unwrap();
        c.transfer(0, dying, 1, SEND, false, 11).unwrap();
        c.transfer(0, healthy, 1, SEND, false, 12).unwrap();
        assert_eq!((c.refs(2), c.refs(3)), (2, 2));
        c.exit(2);
        assert_eq!((c.refs(2), c.refs(3)), (0, 2));
        assert_eq!(c.resolve_send(0, dying), Err(-32));
        assert_eq!(c.close(0, dying), Err(-32));
        assert_eq!(c.stats(1).unwrap().notices, 1);
        let replacement = c.grant(0, 3, SEND).unwrap();
        assert_ne!(replacement, dying);
        assert_eq!(c.resolve_send(0, dying), Err(-116));
        c.exit(0); // previous sender dies; the committed, unrelated notice survives
        let n = c.receive(1).unwrap();
        assert_eq!((n.sender, n.target, n.tag), (0, 3, 12));
        assert_eq!(c.resolve_send(1, n.handle), Ok(3));
        c.exit(3);
        assert_eq!(c.resolve_send(1, n.handle), Err(-32));
        assert_eq!(c.stats(1).unwrap().handles, 0);
        assert_eq!(c.refs(3), 0);
    }

    #[test]
    fn final_generation_is_usable_once_and_retirement_never_wraps() {
        let mut c = CapabilityIpc::new([2, 0, 0]);
        let source = c.grant(0, 2, BOTH).unwrap();
        for slot in &mut c.processes[1].entries {
            slot.generation = LAST_GENERATION;
        }
        c.processes[1].entries[2].generation -= 1;
        c.transfer(0, source, 1, SEND, false, 31).unwrap();
        let last = c.receive(1).unwrap();
        assert_eq!(last.handle >> 4, LAST_GENERATION);
        assert_eq!(c.slot(1, 2).unwrap().generation, LAST_GENERATION);
        c.close(1, last.handle).unwrap();
        let before = c.clone();
        assert_eq!(c.transfer(0, source, 1, SEND, true, 32), Err(-75));
        assert_eq!(c, before);
        assert_eq!(c.resolve_send(1, last.handle), Err(-116));
        assert_eq!(c.resolve_send(0, source), Ok(2));
    }
}
