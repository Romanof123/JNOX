//! Serialized CPU and task ownership for one affinity domain. The caller
//! publishes the saved frame under the same lock as retirement and dispatch.

use super::domain_policy::AffinityDomain;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Lease {
    pub(crate) task: u8,
    pub(crate) cpu: u8,
    pub(crate) sequence: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum Disposition {
    Ready,
    Dormant,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Dormant,
    Ready,
    Running(Lease),
}

#[cfg_attr(jnox_pi4_migration, derive(Clone))]
pub(crate) struct Ownership<const N: usize> {
    affinity: AffinityDomain<N>,
    state: [State; N],
    charged: [u64; N],
    sequence: u64,
}

impl<const N: usize> Ownership<N> {
    pub(crate) fn new(allowed: [u8; N]) -> Option<Self> {
        Some(Self {
            affinity: AffinityDomain::new(allowed)?,
            state: [State::Dormant; N],
            charged: [0; N],
            sequence: 0,
        })
    }

    pub(crate) fn release(&mut self, mask: u32) -> bool {
        if mask == 0 || (u64::from(mask) >> N) != 0 {
            return false;
        }
        if self
            .state
            .iter()
            .enumerate()
            .any(|(id, &state)| mask & (1 << id) != 0 && state != State::Dormant)
        {
            return false;
        }
        for (id, state) in self.state.iter_mut().enumerate() {
            if mask & (1 << id) != 0 {
                *state = State::Ready;
            }
        }
        true
    }

    fn occupied(&self) -> u8 {
        self.state.iter().fold(0, |cpus, state| match state {
            State::Running(lease) => cpus | (1 << lease.cpu),
            _ => cpus,
        })
    }

    pub(crate) fn plan(&self, online: u8, ranked: &[u8]) -> Option<[Option<u8>; 8]> {
        if ranked.len() > N
            || ranked
                .iter()
                .any(|&id| self.state.get(usize::from(id)) != Some(&State::Ready))
        {
            return None;
        }
        self.affinity.place(online & !self.occupied(), ranked)
    }

    /// The dispatcher claims a placement on an online processor. A fresh
    /// sequence distinguishes subsequent grants even on the same processor.
    pub(crate) fn claim(&mut self, cpu: u8, task: u8) -> Option<Lease> {
        if !self.affinity.allows(cpu, task)
            || self.state[usize::from(task)] != State::Ready
            || self.occupied() & (1 << cpu) != 0
        {
            return None;
        }
        let sequence = self.sequence.checked_add(1)?;
        let lease = Lease {
            task,
            cpu,
            sequence,
        };
        self.sequence = sequence;
        self.state[usize::from(task)] = State::Running(lease);
        Some(lease)
    }

    pub(crate) fn retire(&mut self, lease: Lease, units: u64, disposition: Disposition) -> bool {
        let id = usize::from(lease.task);
        if self.state.get(id) != Some(&State::Running(lease)) {
            return false;
        }
        let Some(total) = self.charged[id].checked_add(units) else {
            return false;
        };
        self.charged[id] = total;
        self.state[id] = match disposition {
            Disposition::Ready => State::Ready,
            Disposition::Dormant => State::Dormant,
        };
        true
    }

    pub(crate) fn charged(&self, task: u8) -> Option<u64> {
        self.charged.get(usize::from(task)).copied()
    }
}
