//! Single-slot entitlement arbitration. Only the runtime can attest that a
//! selected frame ran, consumed measurable execution, or trapped.
use super::service_policy::PeriodicReservation;
use super::temporal_policy::{AdmissionError, CbsError, HardCbs, admit_reservations};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Policy {
    Aligned,
    Cbs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Manifest {
    W,
    S,
    R,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Action {
    Long = 0,
    Block = 1,
    Exit = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum State {
    Unreleased = 0,
    Ready = 1,
    Saved = 2,
    Blocked = 3,
    Exited = 4,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Error {
    Invalid = 1,
    OverCapacity = 2,
    Overflow = 3,
    Unexpected = 4,
}

impl From<CbsError> for Error {
    fn from(e: CbsError) -> Self {
        if e == CbsError::TimeOverflow {
            Self::Overflow
        } else {
            Self::Unexpected
        }
    }
}

pub(crate) fn admission(params: &[(u32, u32)]) -> Result<(), Error> {
    if params.is_empty() {
        return Err(Error::Invalid);
    }
    admit_reservations(params).map_err(|e| match e {
        AdmissionError::Invalid => Error::Invalid,
        AdmissionError::OverCapacity => Error::OverCapacity,
        AdmissionError::Overflow => Error::Overflow,
    })
}

impl Manifest {
    pub(crate) fn horizon(self) -> u64 {
        match self {
            Self::W => 34,
            Self::S | Self::R => 10,
        }
    }
    fn params(self) -> [(u32, u32); 2] {
        match self {
            Self::W => [(2, 8), (1, 4)],
            Self::S | Self::R => [(1, 2); 2],
        }
    }
    fn release(self, slot: u64) -> [u32; 2] {
        match (self, slot) {
            (Self::W, 0 | 4 | 5 | 13 | 22) => [1, 0],
            (Self::W, 1) => [0, 100],
            (Self::W, 23) => [100, 0],
            (Self::S, 0) => [4, 4],
            _ => [0, 0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Clock {
    pub(crate) epoch: u64,
    pub(crate) slot_ticks: u64,
    pub(crate) millisecond: u64,
}
impl Clock {
    pub(crate) fn new(epoch: u64, frequency: u64, manifest: Manifest) -> Result<Self, Error> {
        if frequency == 0 || frequency % 1000 != 0 {
            return Err(Error::Invalid);
        }
        let clock = Self {
            epoch,
            slot_ticks: frequency / 20,
            millisecond: frequency / 1000,
        };
        let horizon = manifest.horizon();
        // Union of required deadlines for these two fixed policies. W/aligned
        // refills A at 32 through 40 (CBS through 38); S/aligned through 10.
        let latest_deadline = match manifest {
            Manifest::W => 40,
            Manifest::S | Manifest::R => 10,
        };
        clock.boundary(latest_deadline)?;
        let end = clock.boundary(horizon)?;
        let delay = clock.millisecond.checked_mul(2).ok_or(Error::Overflow)?;
        let latest = end.checked_add(delay).ok_or(Error::Overflow)?;
        charge(0, epoch, latest)?;
        horizon
            .checked_mul(clock.slot_ticks)
            .ok_or(Error::Overflow)?;
        Ok(clock)
    }
    pub(crate) fn boundary(&self, slot: u64) -> Result<u64, Error> {
        slot.checked_mul(self.slot_ticks)
            .and_then(|d| self.epoch.checked_add(d))
            .ok_or(Error::Overflow)
    }
    pub(crate) fn cutoff(&self, slot: u64) -> Result<u64, Error> {
        self.boundary(slot.checked_add(1).ok_or(Error::Overflow)?)?
            .checked_sub(self.millisecond)
            .ok_or(Error::Overflow)
    }
}

pub(crate) fn charge(total: u64, start: u64, stop: u64) -> Result<u64, Error> {
    let elapsed = stop.checked_sub(start).ok_or(Error::Unexpected)?;
    total.checked_add(elapsed).ok_or(Error::Overflow)
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Allocation {
    pub(crate) q: u64,
    pub(crate) d: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Decision {
    pub(crate) owner: Option<usize>,
    pub(crate) action: Option<Action>,
    pub(crate) releases: [u32; 2],
    pub(crate) before: [Allocation; 2],
    pub(crate) after: [Allocation; 2],
    pub(crate) states: [State; 2],
    pub(crate) demand: [u32; 2],
}

#[derive(Clone)]
enum Backend {
    Aligned([PeriodicReservation; 2]),
    Cbs(HardCbs<2>),
}
#[derive(Clone)]
pub(crate) struct Jobs {
    backend: Backend,
    manifest: Manifest,
    slot: u64,
    demand: [u32; 2],
    state: [State; 2],
    pending: Option<(usize, Action)>,
}

impl Jobs {
    pub(crate) fn new(policy: Policy, manifest: Manifest) -> Result<Self, Error> {
        let params = manifest.params();
        if manifest == Manifest::R {
            admission(&[params[0], params[1], (1, 8)])?;
            return Err(Error::Unexpected);
        }
        admission(&params)?;
        let server = |id: usize| {
            let (q, p) = params[id];
            let offset = u64::from(manifest == Manifest::W && id == 1);
            PeriodicReservation::new(offset, q.into(), p.into()).ok_or(Error::Invalid)
        };
        let backend = match policy {
            Policy::Aligned => Backend::Aligned([server(0)?, server(1)?]),
            Policy::Cbs => Backend::Cbs(HardCbs::new(0, params).ok_or(Error::Invalid)?),
        };
        Ok(Self {
            manifest,
            backend,
            slot: 0,
            demand: [0; 2],
            state: [State::Unreleased; 2],
            pending: None,
        })
    }

    /// Exactly once per observed slot boundary. Keep the prior ledger intact
    /// if a late/duplicate dispatch or arithmetic inconsistency is reported.
    pub(crate) fn open(&mut self, slot: u64) -> Result<Decision, Error> {
        if slot != self.slot || slot >= self.manifest.horizon() || self.pending.is_some() {
            return Err(Error::Unexpected);
        }
        let mut candidate = self.clone();
        let result = candidate.decide(slot)?;
        *self = candidate;
        Ok(result)
    }

    fn decide(&mut self, slot: u64) -> Result<Decision, Error> {
        let releases = self.manifest.release(slot);
        for (id, &amount) in releases.iter().enumerate() {
            if amount == 0 {
                continue;
            }
            if !matches!(self.state[id], State::Unreleased | State::Blocked) || self.demand[id] != 0
            {
                return Err(Error::Unexpected);
            }
            if let Backend::Cbs(cbs) = &mut self.backend {
                cbs.release(id, amount)?;
            }
            self.demand[id] = amount;
            self.state[id] = State::Ready;
        }
        let mut before = [Allocation::default(); 2];
        let owner = match &mut self.backend {
            Backend::Aligned(servers) => {
                for (id, server) in servers.iter().enumerate() {
                    let (q, d) = server.allocation_at(slot).ok_or(Error::Overflow)?;
                    before[id] = Allocation { q, d };
                }
                let owner = (0..2)
                    .filter(|&id| self.demand[id] > 0 && before[id].q > 0)
                    .min_by_key(|&id| (before[id].d, id));
                if let Some(id) = owner {
                    if servers[id].grant_at(slot, 1) != Some(1) {
                        return Err(Error::Unexpected);
                    }
                }
                owner
            }
            Backend::Cbs(cbs) => {
                for (id, allocation) in before.iter_mut().enumerate() {
                    let (q, d) = cbs
                        .available(id)?
                        .or_else(|| cbs.allocation(id))
                        .ok_or(Error::Unexpected)?;
                    *allocation = Allocation { q: q.into(), d };
                }
                cbs.tick()?
            }
        };
        let mut after = before;
        let action = if let Some(id) = owner {
            if !matches!(self.state[id], State::Ready | State::Saved) {
                return Err(Error::Unexpected);
            }
            self.demand[id] = self.demand[id].checked_sub(1).ok_or(Error::Unexpected)?;
            after[id].q = after[id].q.checked_sub(1).ok_or(Error::Unexpected)?;
            let action = match (self.manifest, self.demand[id]) {
                (Manifest::W, 0) => Action::Block,
                (Manifest::S, 0) => Action::Exit,
                _ => Action::Long,
            };
            self.pending = Some((id, action));
            Some(action)
        } else {
            None
        };
        self.slot = self.slot.checked_add(1).ok_or(Error::Overflow)?;
        Ok(Decision {
            owner,
            action,
            releases,
            before,
            after,
            states: self.state,
            demand: self.demand,
        })
    }

    /// Called only after the runtime identifies the physical interrupted frame
    /// and trap kind. The policy cannot infer a saved/blocked/exited context.
    pub(crate) fn trap(&mut self, owner: usize, action: Action) -> Result<(), Error> {
        if self.pending != Some((owner, action)) {
            return Err(Error::Unexpected);
        }
        self.state[owner] = match action {
            Action::Long => State::Saved,
            Action::Block => State::Blocked,
            Action::Exit => State::Exited,
        };
        self.pending = None;
        Ok(())
    }
}
