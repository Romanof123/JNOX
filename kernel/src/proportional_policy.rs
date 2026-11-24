//! Single-CPU ticket lottery and service-charged stride. One mask bit per ID
//! declares eligibility. Selection opens a dispatch and stop closes it; the
//! caller serializes eligibility changes at boundaries between dispatches.

use super::feedback_policy::lottery_winner;

// Same exact integer scale as service_policy's equal-quantum StrideClass.
const STRIDE_SCALE: u64 = 720_720; // LCM(1..=16)
const MAX_WEIGHT: u8 = 16;
const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// SplitMix64 with a caller-published initial state. The Weyl increment and
/// both mix multipliers are odd, and xor-right-shifts are invertible. A full
/// state cycle therefore emits every 64-bit word once. Used for repeatable
/// scheduling experiments; it has no cryptographic unpredictability contract.
fn next_word(state: &mut u64) -> u64 {
    *state = state.wrapping_add(GAMMA);
    let mut x = *state;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Drop the short prefix of length 2^64 mod total. The accepted interval's
/// length is divisible by total, so each remainder has equal multiplicity.
/// The permutation cannot repeat a rejected word: after at most cutoff
/// rejections the next sample must succeed. Here 1 <= total <= 32 * 16.
fn uniform_ticket(state: &mut u64, total: u32) -> u32 {
    let bound = u64::from(total);
    let cutoff = bound.wrapping_neg() % bound;
    for _ in 0..=cutoff {
        let word = next_word(state);
        if word >= cutoff {
            return (word % bound) as u32;
        }
    }
    unreachable!("SplitMix64 exhausted the rejected prefix")
}

enum Method<const N: usize> {
    Lottery { state: u64, last_draw: Option<(u32, u32)> },
    Stride { pass: [u64; N], last_base: Option<u64> },
}

pub(crate) struct Proportional<const N: usize> {
    weights: [u8; N],
    ready: u32,
    running: Option<usize>,
    method: Method<N>,
}

impl<const N: usize> Proportional<N> {
    fn new(weights: [u8; N], method: Method<N>) -> Option<Self> {
        if N == 0
            || N > u32::BITS as usize
            || weights.iter().any(|w| !(1..=MAX_WEIGHT).contains(w))
        {
            return None;
        }
        Some(Self { weights, ready: 0, running: None, method })
    }

    pub(crate) fn lottery(weights: [u8; N], seed: u64) -> Option<Self> {
        Self::new(weights, Method::Lottery { state: seed, last_draw: None })
    }

    pub(crate) fn stride(weights: [u8; N]) -> Option<Self> {
        Self::new(weights, Method::Stride { pass: [0; N], last_base: None })
    }

    /// Includes the current task until the next boundary changes its bit.
    pub(crate) fn ready_mask(&self) -> u32 {
        self.ready
    }

    pub(crate) fn set_ready_mask(&mut self, mask: u32) -> bool {
        let valid = u32::MAX >> (u32::BITS as usize - N);
        if self.running.is_some() || mask & !valid != 0 {
            return false;
        }
        if let Method::Stride { pass, .. } = &mut self.method {
            let floor = pass.iter().enumerate()
                .filter(|(id, _)| self.ready & (1u32 << *id) != 0)
                .map(|(_, &value)| value)
                .min()
                .unwrap_or(0);
            for (id, value) in pass.iter_mut().enumerate() {
                let bit = 1u32 << id;
                if mask == 0 {
                    // Preserve the old virtual-time floor across an idle gap.
                    *value = value.saturating_sub(floor);
                } else if mask & !self.ready & bit != 0 {
                    // No credit for missed CPU time. Positive debt survives.
                    *value = (*value).max(floor);
                }
            }
        }
        self.ready = mask;
        true
    }

    fn tickets(weights: &[u8; N], ready: u32) -> [u32; N] {
        core::array::from_fn(|id| {
            if ready & (1u32 << id) == 0 {
                0
            } else {
                u32::from(weights[id])
            }
        })
    }

    /// Pure inspection of the current lottery intervals, ordered by task ID.
    pub(crate) fn winner_for_draw(&self, draw: u32) -> Option<usize> {
        if matches!(self.method, Method::Lottery { .. }) {
            lottery_winner(&Self::tickets(&self.weights, self.ready), u64::from(draw))
        } else {
            None
        }
    }

    /// The accepted ticket and eligible total from the last successful draw.
    pub(crate) fn last_draw(&self) -> Option<(u32, u32)> {
        match &self.method {
            Method::Lottery { last_draw, .. } => *last_draw,
            Method::Stride { .. } => None,
        }
    }

    pub(crate) fn pass(&self, id: usize) -> Option<u64> {
        match &self.method {
            Method::Stride { pass, .. } => pass.get(id).copied(),
            Method::Lottery { .. } => None,
        }
    }

    pub(crate) fn last_normalization(&self) -> Option<u64> {
        match &self.method {
            Method::Stride { last_base, .. } => *last_base,
            Method::Lottery { .. } => None,
        }
    }

    pub(crate) fn dispatch(&mut self) -> Option<usize> {
        if self.running.is_some() || self.ready == 0 {
            return None;
        }
        let winner = match &mut self.method {
            Method::Lottery { state, last_draw } => {
                let tickets = Self::tickets(&self.weights, self.ready);
                let total = tickets.iter().sum();
                let draw = uniform_ticket(state, total);
                *last_draw = Some((draw, total));
                lottery_winner(&tickets, u64::from(draw)).expect("accepted ticket has an owner")
            }
            Method::Stride { pass, last_base } => {
                let winner = (0..N)
                    .filter(|&id| self.ready & (1u32 << id) != 0)
                    .min_by_key(|&id| (pass[id], id))?;
                let base = pass[winner];
                for value in pass {
                    // Only sleepers can fall below the floor. Ready passes and
                    // positive sleeper debt move by exactly the common offset.
                    *value = value.saturating_sub(base);
                }
                *last_base = Some(base);
                winner
            }
        };
        self.running = Some(winner);
        Some(winner)
    }

    /// Caller-supplied actual integer service for this dispatch. Lottery
    /// ratios describe selections; equal service intervals convert selections
    /// into service shares. Stride directly charges the reported service.
    pub(crate) fn stop(&mut self, id: usize, service: u32) -> bool {
        if self.running != Some(id) {
            return false;
        }
        if let Method::Stride { pass, .. } = &mut self.method {
            // Normalization set the winner to zero. The product is bounded by
            // 720720 * (2^32 - 1), and charging never increases another pass.
            pass[id] = (STRIDE_SCALE / u64::from(self.weights[id])) * u64::from(service);
        }
        self.running = None;
        true
    }
}
