//! Single-CPU ticket lottery and service-charged stride. One mask bit per ID
//! declares eligibility. Selection opens a dispatch and stop closes it; the
//! caller serializes eligibility changes at boundaries between dispatches.

use super::feedback_policy::lottery_winner;
use core::convert::TryFrom;

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
#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, jnox_ordinary_prop, jnox_native_proportional)), allow(dead_code))]
pub(crate) struct Draw {
    pub(crate) before: u64, pub(crate) after: u64, pub(crate) words: u64,
    pub(crate) raw: u64, pub(crate) ticket: u32, pub(crate) total: u32,
}

pub(crate) fn reduce_word(word: u64, total: u32) -> Option<u32> {
    if total == 0 { return None }
    let bound = u64::from(total);
    (word >= bound.wrapping_neg() % bound).then(|| (word % bound) as u32)
}

fn uniform_ticket(state: &mut u64, total: u32) -> Option<Draw> {
    if total == 0 { return None }
    let before = *state;
    let bound = u64::from(total);
    let cutoff = bound.wrapping_neg() % bound;
    for attempt in 0..=cutoff {
        let word = next_word(state);
        if let Some(ticket) = reduce_word(word,total) {
            return Some(Draw {before,after:*state,words:attempt+1,raw:word,ticket,total});
        }
    }
    None
}

/// Stable UART predicate codes. Operands retain both halves of stride arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rejection(pub(crate) u64, pub(crate) u128, pub(crate) u128, pub(crate) u128);

impl Rejection {
    pub(crate) fn words(self, site: u64) -> [u64; 8] {
        [site, self.0, self.1 as u64, (self.1 >> 64) as u64,
         self.2 as u64, (self.2 >> 64) as u64, self.3 as u64, (self.3 >> 64) as u64]
    }
}

/// The first parked transition is retained independently of the normal event journal.
#[derive(Default)]
pub(crate) struct FirstRejection(pub(crate) Option<[u64; 8]>);
impl FirstRejection {
    pub(crate) fn record(&mut self, site: u64, reason: Rejection) -> &[u64; 8] {
        self.0.get_or_insert_with(|| reason.words(site))
    }
}

enum Method<const N: usize> {
    Lottery { state: u64, last_draw: Option<(u32,u32)> },
    Stride { pass: [u128; N], last_base: Option<u128> },
}

pub(crate) struct Proportional<const N: usize> {
    weights: [u8; N],
    ready: u32,
    running: Option<usize>,
    method: Method<N>,
    #[cfg(any(test, jnox_ordinary_prop, jnox_native_proportional))]
    audit: Option<Draw>,
}

impl<const N: usize> Proportional<N> {
    fn new(weights: [u8; N], method: Method<N>) -> Option<Self> {
        if N == 0
            || N > u32::BITS as usize
            || weights.iter().any(|w| !(1..=MAX_WEIGHT).contains(w))
        {
            return None;
        }
        Some(Self { weights, ready: 0, running: None, method, #[cfg(any(test, jnox_ordinary_prop, jnox_native_proportional))] audit: None })
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
        self.try_mask(mask).is_ok()
    }

    pub(crate) fn try_mask(&mut self, mask:u32)->Result<(),Rejection> {
        let valid = u32::MAX >> (u32::BITS as usize - N);
        if let Some(id) = self.running { return Err(Rejection(1,id as u128,u64::MAX as u128,mask as u128)); }
        if mask & !valid != 0 {
            return Err(Rejection(2, mask as u128, valid as u128, 0));
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
        Ok(())
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

    #[cfg(any(test, jnox_ordinary_prop, jnox_native_proportional))]
    pub(crate) fn lottery_state(&self) -> Option<u64> {
        match self.method { Method::Lottery {state,..} => Some(state), _ => None }
    }

    #[cfg(any(test, jnox_ordinary_prop, jnox_native_proportional))]
    pub(crate) fn draw_record(&self) -> Option<Draw> {
        self.audit
    }

    pub(crate) fn pass(&self, id: usize) -> Option<u64> {
        u64::try_from(self.pass_wide(id)?).ok()
    }

    pub(crate) fn pass_wide(&self, id: usize) -> Option<u128> {
        match &self.method {
            Method::Stride { pass, .. } => pass.get(id).copied(),
            Method::Lottery { .. } => None,
        }
    }

    pub(crate) fn last_normalization(&self) -> Option<u64> {
        u64::try_from(self.last_normalization_wide()?).ok()
    }

    pub(crate) fn last_normalization_wide(&self) -> Option<u128> {
        match &self.method {
            Method::Stride { last_base, .. } => *last_base,
            Method::Lottery { .. } => None,
        }
    }

    pub(crate) fn dispatch(&mut self) -> Option<usize> {
        self.try_dispatch().ok()
    }

    pub(crate) fn try_dispatch(&mut self)->Result<usize,Rejection> {
        if let Some(id) = self.running { return Err(Rejection(3,id as u128,u64::MAX as u128,self.ready as u128)); }
        if self.ready == 0 {
            return Err(Rejection(4,0,1,0));
        }
        let winner = match &mut self.method {
            Method::Lottery { state, last_draw } => {
                let tickets = Self::tickets(&self.weights, self.ready);
                let total = tickets.iter().sum();
                if total == 0 { return Err(Rejection(5,0,1,self.ready as u128)); }
                let before = *state;
                let draw = uniform_ticket(state, total).ok_or_else(|| Rejection(6,before as u128,*state as u128,total as u128))?;
                *last_draw = Some((draw.ticket, draw.total));
                #[cfg(any(test, jnox_ordinary_prop, jnox_native_proportional))]
                { self.audit = Some(draw); }
                lottery_winner(&tickets, u64::from(draw.ticket)).ok_or(Rejection(7,draw.ticket as u128,total as u128,self.ready as u128))?
            }
            Method::Stride { pass, last_base } => {
                let winner = (0..N)
                    .filter(|&id| self.ready & (1u32 << id) != 0)
                    .min_by_key(|&id| (pass[id], id)).ok_or(Rejection(8,self.ready as u128,N as u128,0))?;
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
        Ok(winner)
    }

    /// Caller-supplied actual integer service for this dispatch. Lottery
    /// ratios describe selections; equal service intervals convert selections
    /// into service shares. Stride directly charges the reported service.
    pub(crate) fn stop(&mut self, id: usize, service: u32) -> bool {
        self.stop_elapsed(id, u64::from(service))
    }

    /// Charge one observed lease without truncating a long counter interval.
    /// Dispatch normalized its winner to zero, so even the maximum possible
    /// charge fits in 84 bits. Other ready passes retain their relative debt.
    pub(crate) fn stop_elapsed(&mut self, id: usize, service: u64) -> bool {
        self.try_stop_elapsed(id, service).is_ok()
    }

    pub(crate) fn try_stop_elapsed(&mut self,id:usize,service:u64)->Result<(),Rejection> {
        let owner = self.running.ok_or(Rejection(9,id as u128,u64::MAX as u128,service as u128))?;
        if owner != id {
            return Err(Rejection(10,id as u128,owner as u128,service as u128));
        }
        if let Method::Stride { pass, .. } = &mut self.method {
            let stride = u128::from(STRIDE_SCALE / u64::from(self.weights[id]));
            let increment = stride * u128::from(service);
            let Some(charged) = pass[id].checked_add(increment) else {
                return Err(Rejection(11,pass[id],increment,id as u128));
            };
            pass[id] = charged;
        }
        self.running = None;
        Ok(())
    }
}

/// Finite occupancy contract for three continuously ready streams.
#[cfg(any(test, jnox_ordinary_prop))]
#[derive(Clone, Copy)]
pub(crate) struct WindowInput {
    pub(crate) seed: u64,
    pub(crate) weights: [u8; 3],
    pub(crate) ready: u32,
    pub(crate) horizon: usize,
    pub(crate) width: usize,
    pub(crate) floors: [u8; 3],
}

#[derive(Clone, Copy)]
pub(crate) struct NativeWindowInput<const N: usize> {
    pub(crate) seed: u64,
    pub(crate) weights: [u8; N],
    pub(crate) horizon: usize,
    pub(crate) width: usize,
    pub(crate) floors: [u8; N],
}

#[derive(Debug, PartialEq)]
pub(crate) struct WindowReport<const N: usize = 3> {
    pub(crate) minimum: [u8; N],
    pub(crate) first_short: Option<(usize, usize, u8)>,
}

#[cfg(any(test, jnox_ordinary_prop))]
impl WindowInput {
    pub(crate) fn analyze(self) -> Option<WindowReport> {
        // The original linked fixture admits three ready streams and requires
        // a positive floor for each. Native packages may declare a zero floor.
        if self.ready != 7 || self.floors.contains(&0) {
            return None;
        }
        NativeWindowInput {
            seed: self.seed,
            weights: self.weights,
            horizon: self.horizon,
            width: self.width,
            floors: self.floors,
        }.analyze()
    }
}

const WINDOW_CAPACITY: usize = 64;

impl<const N: usize> NativeWindowInput<N> {

    /// Scratch policy is intentionally distinct from the live policy. Only
    /// aggregates and the first failing predicate can cross this boundary.
    pub(crate) fn analyze(self) -> Option<WindowReport<N>> {
        self.try_analyze().ok()
    }

    pub(crate) fn try_analyze(self)->Result<WindowReport<N>,Rejection> {
        for (code, value, low, high) in [(12,N,1,3),(13,self.horizon,1,WINDOW_CAPACITY),(14,self.width,1,self.horizon)] {
            if !(low..=high).contains(&value) { return Err(Rejection(code,value as u128,low as u128,high as u128)); }
        }
        for (id,&floor) in self.floors.iter().enumerate() {
            if usize::from(floor)>self.width {return Err(Rejection(15,floor as u128,self.width as u128,id as u128));}
        }
        let sum=self.floors.iter().map(|&f| usize::from(f)).sum::<usize>();
        if sum>self.width {return Err(Rejection(16,sum as u128,self.width as u128,0));}
        let mut scratch=Proportional::lottery(self.weights,self.seed).ok_or_else(|| {
            let id=self.weights.iter().position(|&w| !(1..=MAX_WEIGHT).contains(&w)).unwrap();
            Rejection(17,self.weights[id] as u128,MAX_WEIGHT as u128,id as u128)
        })?;
        scratch.try_mask((1u32 << N)-1)?;
        let mut ring = [0usize; WINDOW_CAPACITY];
        let mut occupancy = [0u8; N];
        let mut report = WindowReport { minimum: [u8::MAX; N], first_short: None };
        for slot in 0..self.horizon {
            let index = slot % self.width;
            if slot >= self.width {
                let previous = ring[index];
                occupancy[previous] = occupancy[previous].checked_sub(1).ok_or(Rejection(18,0,previous as u128,slot as u128))?;
            }
            let winner = scratch.try_dispatch()?;
            scratch.try_stop_elapsed(winner,0)?;
            ring[index] = winner;
            occupancy[winner] = occupancy[winner].checked_add(1).ok_or(Rejection(19,255,winner as u128,slot as u128))?;
            if let Some(start) = (slot + 1).checked_sub(self.width) {
                for (id, &count) in occupancy.iter().enumerate() {
                    report.minimum[id] = report.minimum[id].min(count);
                    if count < self.floors[id] && report.first_short.is_none() {
                        report.first_short = Some((start, id, count));
                    }
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod rejection_tests {
    use super::*;

    #[test]
    fn wrong_owner_is_first_and_distinct_from_no_owner() {
        let mut first = FirstRejection::default();
        let mut p = Proportional::stride([1,1]).unwrap();
        assert!(p.set_ready_mask(3));
        assert_eq!(p.dispatch(), Some(0));
        let wrong = p.try_stop_elapsed(1,123).unwrap_err();
        assert_eq!(*first.record(17,wrong),[17,10,1,0,0,0,123,0]);
        assert!(p.stop_elapsed(0,123));
        let absent = p.try_stop_elapsed(1,5).unwrap_err();
        assert_eq!(absent.words(17),[17,9,1,0,u64::MAX,0,5,0]);
        assert_eq!(*first.record(17,absent),[17,10,1,0,0,0,123,0]);
    }

    #[test]
    fn mask_dispatch_window_and_wide_charge_predicates() {
        let mut p = Proportional::stride([1]).unwrap();
        assert_eq!(p.try_dispatch(),Err(Rejection(4,0,1,0)));
        assert_eq!(p.try_mask(2),Err(Rejection(2,2,1,0)));
        p.try_mask(1).unwrap();
        p.try_dispatch().unwrap();
        assert_eq!(p.try_dispatch(),Err(Rejection(3,0,u64::MAX as u128,1)));
        assert_eq!(p.try_mask(0),Err(Rejection(1,0,u64::MAX as u128,0)));
        if let Method::Stride{pass,..}=&mut p.method { pass[0]=u128::MAX; }
        let reason=p.try_stop_elapsed(0,u64::MAX).unwrap_err();
        let increment=u128::from(STRIDE_SCALE)*u128::from(u64::MAX);
        assert_eq!(reason.words(17),[17,11,u64::MAX,u64::MAX,increment as u64,(increment>>64) as u64,0,0]);
        let input=NativeWindowInput{seed:0,weights:[1],horizon:4,width:2,floors:[3]};
        assert_eq!(input.try_analyze(),Err(Rejection(15,3,2,0)));
    }
}
