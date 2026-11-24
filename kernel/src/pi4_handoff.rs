//! Values staged under the migration transaction lock before committing a transition.
use super::smp_policy::Lease;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin { Fresh, IrqSaved }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameInstall {
    pub pointer: usize,
    pub version: u64,
    pub origin: Origin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot<T> { Empty, Pending(T), Consumed(T) }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Opening {
    pub epoch: u64,
    pub retired: Lease,
    pub target: Lease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Reverse {
    pub epoch: u64,
    pub target: Lease,
    pub frame: FrameInstall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handoff {
    pub frames: [Option<FrameInstall>; 3],
    pub opening: Slot<Opening>,
    pub reverse: Slot<Reverse>,
}

#[derive(Clone, Copy)]
pub(crate) struct Receive {
    pub iar: u32,
    /// Discovered GIC target mask of the logical peer for this directed edge.
    pub peer_target: u8,
    pub cpu: usize,
    pub epoch: u64,
    pub claim: Option<Lease>,
    pub live: u64,
    /// Caller checked the interrupted role, frame and dispatcher stack.
    pub trap_valid: bool,
}

impl Receive {
    fn matches(self, epoch: u64, target: Lease, live: u64) -> bool {
        self.peer_target.is_power_of_two() && self.iar & 0x3ff == 1
            && (self.iar >> 10) & 7 == self.peer_target.trailing_zeros()
            && self.cpu == usize::from(target.cpu) && self.epoch == epoch
            && self.claim == Some(target) && self.live == live && self.trap_valid
    }
}

impl Handoff {
    pub const fn new() -> Self {
        Self { frames: [None; 3], opening: Slot::Empty, reverse: Slot::Empty }
    }

    pub fn install(&mut self, task: u8, pointer: usize, origin: Origin) -> Option<FrameInstall> {
        let slot = self.frames.get_mut(usize::from(task))?;
        if pointer == 0 || pointer % 16 != 0 {
            return None;
        }
        let version = slot.map_or(0, |frame| frame.version).checked_add(1)?;
        match *slot {
            None if origin == Origin::Fresh && self.opening == Slot::Empty => {}
            Some(previous) if previous.origin == Origin::Fresh && previous.version == 1
                && matches!(self.opening, Slot::Consumed(_)) && self.reverse == Slot::Empty
                && matches!((task, origin), (0, Origin::IrqSaved) | (2, Origin::Fresh)) => {}
            _ => return None,
        }
        let frame = FrameInstall { pointer, version, origin };
        *slot = Some(frame);
        Some(frame)
    }

    pub fn publish_opening(&mut self, epoch: u64, retired: Lease, target: Lease) -> Option<Opening> {
        if self.opening != Slot::Empty || epoch != 0
            || (retired.cpu, retired.task) != (1, 2) || (target.cpu, target.task) != (0, 0)
            || retired.sequence == 0 || target.sequence == 0 || retired.sequence == target.sequence {
            return None;
        }
        let opening = Opening { epoch, retired, target };
        self.opening = Slot::Pending(opening);
        Some(opening)
    }

    /// retired is the independently recorded completed P1 lease. The caller also
    /// supplies CPU1 current ownership under the same transaction lock.
    pub fn consume_opening(&mut self, rx: Receive, retired: Option<Lease>, source_claim: Option<Lease>, milestone: bool) -> Option<Opening> {
        let Slot::Pending(opening) = self.opening else { return None };
        if source_claim.is_some() || !milestone || retired != Some(opening.retired)
            || !rx.matches(opening.epoch, opening.target, opening.target.sequence) {
            return None;
        }
        self.opening = Slot::Consumed(opening);
        Some(opening)
    }

    fn selection(&self, epoch: u64, target: Lease, global: bool) -> Option<Reverse> {
        let Slot::Consumed(opening) = self.opening else { return None };
        let (task, origin) = if global { (0, Origin::IrqSaved) } else { (2, Origin::Fresh) };
        if epoch != 1 || (target.cpu, target.task) != (1, task)
            || target.sequence <= opening.target.sequence.max(opening.retired.sequence) {
            return None;
        }
        let frame = self.frames[usize::from(task)]?;
        if frame.origin != origin || frame.version < 2 || frame.pointer == 0 || frame.pointer % 16 != 0 {
            return None;
        }
        Some(Reverse { epoch, target, frame })
    }

    pub fn publish_reverse(&mut self, epoch: u64, target: Lease, global: bool) -> Option<Reverse> {
        if self.reverse != Slot::Empty { return None; }
        let reverse = self.selection(epoch, target, global)?;
        self.reverse = Slot::Pending(reverse);
        Some(reverse)
    }

    /// Caller validates the selected frame and role in addition to the idle trap.
    pub fn consume_reverse(&mut self, rx: Receive, global: bool) -> Option<Reverse> {
        let Slot::Pending(reverse) = self.reverse else { return None };
        if !rx.matches(reverse.epoch, reverse.target, 0)
            || self.selection(rx.epoch, rx.claim?, global)? != reverse {
            return None;
        }
        self.reverse = Slot::Consumed(reverse);
        Some(reverse)
    }
}
