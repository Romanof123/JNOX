//! IPC-010 synthetic permit for the single-CPU virtual ENABLE bench.
//! Transport ownership, loan release, release scheduling and device commits
//! are external. The receiver operates only on the copied private frame.
#![no_std]
#![forbid(unsafe_code)]

pub const FRAME_BYTES: usize = 44;
pub const DATA_ID: u16 = 0x0101;
pub const CHANNEL: u16 = 1;
pub const VALIDITY_MS: u32 = 20;
pub const ISSUE_PERIOD_MS: u32 = 10;
pub const CONTROL_PERIOD_MS: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    Length = 1,
    Crc = 2,
    Format = 3,
    Route = 4,
    Slot = 5,
    Clock = 6,
    Generation = 7,
    Sequence = 8,
    Future = 9,
    Expired = 10,
    IssueOrder = 11,
    NotReady = 12,
    Unauthorized = 13,
    Budget = 14,
    Overflow = 15,
    AlreadyArmed = 16,
    NeedsOpen = 17,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permit {
    pub generation: u64,
    pub sequence: u64,
    pub issue: u64,
    pub permit: bool,
}

/// Authenticate endpoints via the kernel first. These arguments are transport
/// metadata, never fields supplied by the wire. Call before copying 44 bytes.
pub fn validate_loan(sender: usize, receiver: usize, slot: usize, published_len: usize) -> Result<(), Error> {
    if sender != 0 || receiver != 1 {
        return Err(Error::Route);
    }
    if slot >= 3 {
        return Err(Error::Slot);
    }
    if published_len != FRAME_BYTES {
        return Err(Error::Length);
    }
    Ok(())
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    !crc
}

pub fn encode(frame: Permit) -> Result<[u8; FRAME_BYTES], Error> {
    if frame.generation == 0 {
        return Err(Error::Generation);
    }
    if frame.sequence == 0 {
        return Err(Error::Sequence);
    }
    let mut b = [0; FRAME_BYTES];
    b[0] = 1;
    b[1] = 1;
    b[2..4].copy_from_slice(&(FRAME_BYTES as u16).to_le_bytes());
    b[4..6].copy_from_slice(&CHANNEL.to_le_bytes());
    b[6..8].copy_from_slice(&DATA_ID.to_le_bytes());
    b[8..16].copy_from_slice(&frame.generation.to_le_bytes());
    b[16..24].copy_from_slice(&frame.sequence.to_le_bytes());
    b[24..32].copy_from_slice(&frame.issue.to_le_bytes());
    b[32..36].copy_from_slice(&VALIDITY_MS.to_le_bytes());
    b[36] = u8::from(frame.permit);
    let crc = crc32(&b[..40]);
    b[40..44].copy_from_slice(&crc.to_le_bytes());
    Ok(b)
}

pub fn decode(private: &[u8]) -> Result<Permit, Error> {
    let b: &[u8; FRAME_BYTES] = private.try_into().map_err(|_| Error::Length)?;
    if b[40..44] != crc32(&b[..40]).to_le_bytes() {
        return Err(Error::Crc);
    }
    if b[2..4] != (FRAME_BYTES as u16).to_le_bytes() {
        return Err(Error::Length);
    }
    if b[0] != 1
        || b[1] != 1
        || b[4..6] != CHANNEL.to_le_bytes()
        || b[6..8] != DATA_ID.to_le_bytes()
        || b[32..36] != VALIDITY_MS.to_le_bytes()
        || b[36] > 1
        || b[37..40] != [0; 3]
    {
        return Err(Error::Format);
    }
    let generation = u64::from_le_bytes(b[8..16].try_into().unwrap());
    let sequence = u64::from_le_bytes(b[16..24].try_into().unwrap());
    if generation == 0 {
        return Err(Error::Generation);
    }
    if sequence == 0 {
        return Err(Error::Sequence);
    }
    Ok(Permit {
        generation,
        sequence,
        issue: u64::from_le_bytes(b[24..32].try_into().unwrap()),
        permit: b[36] == 1,
    })
}

// Tick comparisons never add a deadline to a near-maximum issue tick or
// truncate the fractional tick at a millisecond boundary. More than half
// the counter range between samples is ambiguous for this clock domain.
fn scaled_elapsed(start: u64, end: u64) -> Result<u128, Error> {
    let delta = end.checked_sub(start).ok_or(Error::Clock)?;
    if delta > i64::MAX as u64 {
        return Err(Error::Clock);
    }
    Ok(u128::from(delta) * 1000)
}

pub fn within_ms(start: u64, end: u64, hz: u64, limit_ms: u32) -> Result<bool, Error> {
    if hz == 0 {
        return Err(Error::Clock);
    }
    Ok(scaled_elapsed(start, end)? <= u128::from(hz) * u128::from(limit_ms))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseTiming {
    pub detected_within_25ms: bool,
    pub inhibited_within_5ms: bool,
    pub total_within_30ms: bool,
}

/// The caller supplies independently observed ordered event timestamps.
pub fn response_timing(fault: u64, detected: u64, safe: u64, hz: u64) -> Result<ResponseTiming, Error> {
    Ok(ResponseTiming {
        detected_within_25ms: within_ms(fault, detected, hz, 25)?,
        inhibited_within_5ms: within_ms(detected, safe, hz, 5)?,
        total_within_30ms: within_ms(fault, safe, hz, 30)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: u64,
    pub sequence: u64,
    pub issue: Option<u64>,
    pub permit: bool,
    pub recovery: u8,
    pub enabled: bool,
    pub faulted: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Receiver {
    hz: u64,
    state: Snapshot,
    pair_first: Option<u64>,
    last_clock: Option<u64>,
    used_rearm: bool,
    in_activation: bool,
    frame_attempts: u8,
    command_attempted: bool,
}

impl Receiver {
    pub const fn new(counter_hz: u64) -> Result<Self, Error> {
        if counter_hz == 0 {
            return Err(Error::Clock);
        }
        Ok(Self {
            hz: counter_hz,
            state: Snapshot {
                generation: 0,
                sequence: 0,
                issue: None,
                permit: false,
                recovery: 0,
                enabled: false,
                faulted: false,
            },
            pair_first: None,
            last_clock: None,
            used_rearm: false,
            in_activation: false,
            frame_attempts: 0,
            command_attempted: false,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state
    }

    /// Explicit transport restart invalidates this generation. Silent input
    /// loss is instead detected by the independent periodic freshness check.
    pub fn disconnect(&mut self) {
        self.latch();
    }

    fn latch(&mut self) -> bool {
        let newly_latched = !self.state.faulted;
        self.state.faulted = true;
        self.state.enabled = false;
        newly_latched
    }

    fn fresh(&self, issue: u64, now: u64) -> Result<bool, Error> {
        Ok(scaled_elapsed(issue, now)? < u128::from(self.hz) * u128::from(VALIDITY_MS))
    }

    /// Sample even when no message is available. True marks a newly latched
    /// expiry; the last accepted data, sequence and recovery evidence remain.
    pub fn check(&mut self, now: u64) -> Result<bool, Error> {
        if let Some(last) = self.last_clock {
            if scaled_elapsed(last, now).is_err() {
                self.latch();
                return Err(Error::Clock);
            }
        }
        self.last_clock = Some(now);
        if let Some(issue) = self.state.issue {
            match self.fresh(issue, now) {
                Ok(true) => {}
                Ok(false) => return Ok(self.latch()),
                Err(error) => {
                    self.latch();
                    return Err(error);
                }
            }
        }
        Ok(false)
    }

    pub fn begin_activation(&mut self, now: u64) -> Result<bool, Error> {
        self.frame_attempts = 0;
        self.command_attempted = false;
        self.in_activation = true;
        self.check(now)
    }

    fn command(&mut self, issuer: usize, now: u64) -> Result<(), Error> {
        self.check(now)?;
        if !self.in_activation || self.command_attempted {
            return Err(Error::Budget);
        }
        self.command_attempted = true;
        if issuer != 2 {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    pub fn open(&mut self, issuer: usize, generation: u64, now: u64) -> Result<(), Error> {
        self.command(issuer, now)?;
        let next = self.state.generation.checked_add(1).ok_or(Error::Overflow)?;
        if generation != next {
            return Err(Error::Generation);
        }
        self.state = Snapshot {
            generation,
            sequence: 0,
            issue: None,
            permit: false,
            recovery: 0,
            enabled: false,
            faulted: false,
        };
        self.pair_first = None;
        self.used_rearm = false;
        Ok(())
    }

    /// Verify only private bytes. Sample the live clock after decoding and
    /// before committing all data fields. Independent expiry wins on equality.
    pub fn accept(&mut self, private_frame: &[u8], clock: impl FnOnce() -> u64) -> Result<(), Error> {
        if !self.in_activation || self.frame_attempts == 2 {
            return Err(Error::Budget);
        }
        self.frame_attempts += 1;
        let decoded = decode(private_frame);
        let now = clock();
        let expired = self.check(now)?;
        if expired {
            return Err(Error::Expired);
        }
        if self.state.faulted || self.state.generation == 0 {
            return Err(Error::NeedsOpen);
        }
        let frame = decoded?;
        if frame.generation != self.state.generation {
            return Err(Error::Generation);
        }
        if frame.sequence != self.state.sequence.checked_add(1).ok_or(Error::Overflow)? {
            return Err(Error::Sequence);
        }
        if frame.issue > now {
            return Err(Error::Future);
        }
        if !self.fresh(frame.issue, now)? {
            return Err(Error::Expired);
        }
        if self.state.issue.is_some_and(|last| frame.issue <= last) {
            return Err(Error::IssueOrder);
        }

        // Keep the most recent qualifying pair. A fresh, correctly sequenced
        // set sample at another spacing can start a new pair.
        let mut pair_first = None;
        if self.state.permit && frame.permit {
            if let Some(previous) = self.state.issue {
                if scaled_elapsed(previous, frame.issue)? == u128::from(self.hz) * u128::from(ISSUE_PERIOD_MS)
                {
                    pair_first = Some(previous);
                }
            }
        }
        self.state.sequence = frame.sequence;
        self.state.issue = Some(frame.issue);
        self.state.permit = frame.permit;
        self.state.recovery = if pair_first.is_some() {
            2
        } else {
            u8::from(frame.permit)
        };
        self.pair_first = pair_first;
        if !frame.permit {
            self.state.enabled = false;
        }
        Ok(())
    }

    pub fn rearm(&mut self, issuer: usize, generation: u64, now: u64) -> Result<(), Error> {
        self.command(issuer, now)?;
        if self.state.faulted || self.state.generation == 0 {
            return Err(Error::NeedsOpen);
        }
        if generation != self.state.generation {
            return Err(Error::Generation);
        }
        if self.used_rearm {
            return Err(Error::AlreadyArmed);
        }
        let first = self.pair_first.ok_or(Error::NotReady)?;
        if !self.fresh(first, now)? {
            return Err(Error::NotReady);
        }
        // check() already confirmed the second permit was fresh at this tick.
        self.used_rearm = true;
        self.state.enabled = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(generation: u64, sequence: u64, issue: u64, permit: bool) -> [u8; FRAME_BYTES] {
        encode(Permit {
            generation,
            sequence,
            issue,
            permit,
        })
        .unwrap()
    }

    fn opened() -> Receiver {
        let mut r = Receiver::new(1000).unwrap();
        assert!(!r.snapshot().enabled);
        r.begin_activation(0).unwrap();
        r.open(2, 1, 0).unwrap();
        r
    }

    fn pair() -> Receiver {
        let mut r = opened();
        r.accept(&frame(1, 1, 0, true), || 0).unwrap();
        r.begin_activation(10).unwrap();
        r.accept(&frame(1, 2, 10, true), || 10).unwrap();
        r
    }

    #[test]
    fn serialization_matches_frozen_independent_vector() {
        let golden = [
            1, 1, 44, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            20, 0, 0, 0, 1, 0, 0, 0, 0x70, 0x6e, 0x56, 0xe3,
        ];
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(frame(1, 1, 0, true), golden);
        assert_eq!(
            decode(&golden),
            Ok(Permit {
                generation: 1,
                sequence: 1,
                issue: 0,
                permit: true
            })
        );
        for index in [0, 1, 2, 4, 6, 32, 36, 37, 38, 39] {
            let mut bad = golden;
            bad[index] = 0xff;
            let recomputed = crc32(&bad[..40]);
            bad[40..].copy_from_slice(&recomputed.to_le_bytes());
            assert!(decode(&bad).is_err());
        }
        assert_eq!(decode(&golden[..43]), Err(Error::Length));
        let mut damaged = golden;
        damaged[36] ^= 1;
        assert_eq!(decode(&damaged), Err(Error::Crc));
        assert_eq!(validate_loan(0, 1, 2, 44), Ok(()));
        assert_eq!(validate_loan(2, 1, 2, 44), Err(Error::Route));
        assert_eq!(validate_loan(0, 1, 3, 44), Err(Error::Slot));
        assert_eq!(validate_loan(0, 1, 1, 45), Err(Error::Length));
    }

    #[test]
    fn expiry_before_same_tick_frame_preserves_the_last_accepted_data() {
        let mut r = pair();
        r.begin_activation(15).unwrap();
        r.rearm(2, 1, 15).unwrap();
        assert!(r.snapshot().enabled);
        let before = r.snapshot();
        assert_eq!(r.begin_activation(29), Ok(false));
        assert_eq!(r.accept(&frame(1, 3, 30, true), || 30), Err(Error::Expired));
        let after = r.snapshot();
        assert_eq!(
            after,
            Snapshot {
                enabled: false,
                faulted: true,
                ..before
            }
        );
        assert_eq!(r.check(34), Ok(false));
        r.begin_activation(35).unwrap();
        assert_eq!(r.accept(&frame(1, 3, 35, true), || 35), Err(Error::NeedsOpen));
        assert_eq!(r.rearm(2, 1, 35), Err(Error::NeedsOpen));
        r.begin_activation(40).unwrap();
        r.open(2, 2, 40).unwrap();
        assert_eq!(r.snapshot().issue, None);
        assert_eq!(r.accept(&frame(1, 3, 40, true), || 40), Err(Error::Generation));
    }

    #[test]
    fn both_samples_must_be_fresh_at_a_separate_authorized_rearm() {
        let mut r = pair();
        let mut late = r;
        late.begin_activation(20).unwrap();
        assert_eq!(late.rearm(2, 1, 20), Err(Error::NotReady));
        assert!(!late.snapshot().enabled);
        r.begin_activation(15).unwrap();
        assert_eq!(r.rearm(1, 1, 15), Err(Error::Unauthorized));
        assert_eq!(r.rearm(2, 1, 15), Err(Error::Budget));
        r.begin_activation(16).unwrap();
        r.rearm(2, 1, 16).unwrap();
        r.begin_activation(17).unwrap();
        assert_eq!(r.rearm(2, 1, 17), Err(Error::AlreadyArmed));
        r.accept(&frame(1, 3, 17, false), || 17).unwrap();
        assert!(!r.snapshot().enabled);
        assert_eq!(r.snapshot().recovery, 0);
        r.begin_activation(20).unwrap();
        r.accept(&frame(1, 4, 20, true), || 20).unwrap();
        r.begin_activation(30).unwrap();
        r.accept(&frame(1, 5, 30, true), || 30).unwrap();
        assert_eq!(r.rearm(2, 1, 30), Err(Error::AlreadyArmed));
    }

    #[test]
    fn rejected_input_never_advances_recovery_or_refreshes_an_issue() {
        let mut r = opened();
        r.accept(&frame(1, 1, 0, true), || 0).unwrap();
        r.begin_activation(5).unwrap();
        assert_eq!(r.rearm(2, 1, 5), Err(Error::NotReady));
        let before = r.snapshot();
        for (wire, error) in [
            (frame(1, 1, 4, true), Error::Sequence),
            (frame(1, 3, 4, true), Error::Sequence),
            (frame(2, 2, 4, true), Error::Generation),
            (frame(1, 2, 0, true), Error::IssueOrder),
            (frame(1, 2, 6, true), Error::Future),
        ] {
            r.begin_activation(5).unwrap();
            assert_eq!(r.accept(&wire, || 5), Err(error));
            assert_eq!(r.snapshot(), before);
        }
        r.begin_activation(10).unwrap();
        r.accept(&frame(1, 2, 10, true), || 10).unwrap();
        assert!(!r.snapshot().enabled); // no saved premature REARM
        assert_eq!(r.accept(&[0; 44], || 10), Err(Error::Crc));
        assert_eq!(r.accept(&frame(1, 3, 11, true), || 11), Err(Error::Budget));
        r.disconnect();
        assert_eq!(r.snapshot().issue, Some(10));
        r.begin_activation(12).unwrap();
        assert_eq!(r.accept(&frame(1, 3, 12, true), || 12), Err(Error::NeedsOpen));
    }

    #[test]
    fn integer_clock_boundaries_and_counter_exhaustion() {
        let mut r = Receiver::new(1001).unwrap();
        r.begin_activation(0).unwrap();
        r.open(2, 1, 0).unwrap();
        r.accept(&frame(1, 1, 0, true), || 0).unwrap();
        assert_eq!(r.check(20), Ok(false)); // 20 ticks / 1001 Hz is less than 20 ms
        assert_eq!(r.check(21), Ok(true));
        assert_eq!(r.check(20), Err(Error::Clock));
        let mut near = opened();
        near.state.sequence = u64::MAX;
        assert_eq!(near.accept(&frame(1, 1, 0, true), || 0), Err(Error::Overflow));
        near.state.generation = u64::MAX;
        near.begin_activation(1).unwrap();
        assert_eq!(near.open(2, 1, 1), Err(Error::Overflow));
        assert_eq!(within_ms(1, 6, 1000, 5), Ok(true));
        assert_eq!(within_ms(1, 7, 1000, 5), Ok(false));
        assert_eq!(within_ms(u64::MAX, 0, 1000, 5), Err(Error::Clock));
        assert_eq!(
            response_timing(0, 25, 30, 1000),
            Ok(ResponseTiming {
                detected_within_25ms: true,
                inhibited_within_5ms: true,
                total_within_30ms: true
            })
        );
        let late = response_timing(0, 26, 32, 1000).unwrap();
        assert!(!late.detected_within_25ms && !late.inhibited_within_5ms && !late.total_within_30ms);
    }
}
