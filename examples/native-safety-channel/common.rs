use core::time::Duration;
use jnox_native::{ipc, monotonic_time, safety_channel as dev, sleep_until};

pub type Bench<T> = Result<T, i32>;
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Case {
    Healthy = 1,
    Silence,
    Invalid,
    Saturation,
    Delayed,
    Restart,
    Collision,
    Phase,
    Early,
    Boundary,
    NoRearm,
    One,
    Queued,
    BadSpacing,
    Observed,
    LiveStartup,
    BadCrc,
    BadHeaderCrc,
    BadLength,
    BadVersion,
    BadType,
    BadChannel,
    BadDataId,
    BadValidity,
    BadPermit,
    BadReserved,
    Duplicate,
    Reordered,
    Gap,
    OldGeneration,
    FutureIssue,
    ExpiredIssue,
    IssueOrder,
    WrongOwner,
    Rearm15,
    Rearm20,
    RecoverLatch,
    RecoverHold,
}
impl Case {
    pub fn rearm_target_ms(self) -> Option<u64> {
        match self {
            Self::Rearm15 => Some(15),
            Self::Rearm20 => Some(20),
            _ => None,
        }
    }
    pub fn active_case(self) -> bool {
        matches!(
            self,
            Self::BadCrc
                | Self::BadHeaderCrc
                | Self::BadLength
                | Self::BadVersion
                | Self::BadType
                | Self::BadChannel
                | Self::BadDataId
                | Self::BadValidity
                | Self::BadPermit
                | Self::BadReserved
                | Self::Duplicate
                | Self::Reordered
                | Self::Gap
                | Self::OldGeneration
                | Self::FutureIssue
                | Self::ExpiredIssue
                | Self::IssueOrder
                | Self::WrongOwner
        )
    }
    pub fn active_offset_ms(self) -> u64 {
        if self == Self::OldGeneration {
            40
        } else {
            0
        }
    }
    pub fn active_generation(self) -> u64 {
        if self == Self::OldGeneration {
            2
        } else {
            1
        }
    }
    pub unsafe fn parse(argc: i32, argv: *const *const u8) -> Bench<Self> {
        if argc != 2 || argv.is_null() {
            return Err(81);
        }
        let arg = *argv.add(1);
        if arg.is_null() {
            return Err(81);
        }
        let mut n = 0;
        while n < 16 && *arg.add(n) != 0 {
            n += 1
        }
        if n == 16 {
            return Err(81);
        }
        match core::slice::from_raw_parts(arg, n) {
            b"healthy" => Ok(Self::Healthy),
            b"silence" => Ok(Self::Silence),
            b"invalid" => Ok(Self::Invalid),
            b"saturation" => Ok(Self::Saturation),
            b"delayed" => Ok(Self::Delayed),
            b"restart" => Ok(Self::Restart),
            b"collision" => Ok(Self::Collision),
            b"phase" => Ok(Self::Phase),
            b"early" => Ok(Self::Early),
            b"boundary" => Ok(Self::Boundary),
            b"no-rearm" => Ok(Self::NoRearm),
            b"one" => Ok(Self::One),
            b"queued" => Ok(Self::Queued),
            b"bad-spacing" => Ok(Self::BadSpacing),
            b"observed" => Ok(Self::Observed),
            b"live-startup" => Ok(Self::LiveStartup),
            b"bad-crc" => Ok(Self::BadCrc),
            b"bad-header-crc" => Ok(Self::BadHeaderCrc),
            b"bad-length" => Ok(Self::BadLength),
            b"bad-version" => Ok(Self::BadVersion),
            b"bad-type" => Ok(Self::BadType),
            b"bad-channel" => Ok(Self::BadChannel),
            b"bad-data-id" => Ok(Self::BadDataId),
            b"bad-validity" => Ok(Self::BadValidity),
            b"bad-permit" => Ok(Self::BadPermit),
            b"bad-reserved" => Ok(Self::BadReserved),
            b"duplicate" => Ok(Self::Duplicate),
            b"reordered" => Ok(Self::Reordered),
            b"gap" => Ok(Self::Gap),
            b"old-generation" => Ok(Self::OldGeneration),
            b"future-issue" => Ok(Self::FutureIssue),
            b"expired-issue" => Ok(Self::ExpiredIssue),
            b"issue-order" => Ok(Self::IssueOrder),
            b"wrong-owner" => Ok(Self::WrongOwner),
            b"rearm-15" => Ok(Self::Rearm15),
            b"rearm-20" => Ok(Self::Rearm20),
            b"recover-latch" => Ok(Self::RecoverLatch),
            b"recover-hold" => Ok(Self::RecoverHold),
            _ => Err(81),
        }
    }
}
pub const ACK: u64 = 0x5341464500000000;
pub const READY: u64 = 0x7265616479;
pub const STOP: u64 = 0x73746f70;
pub const LAST_SAMPLE: u64 = 0x6c617374;
pub const RELEASE_INPUT: u64 = 0x696e707574;
pub const GRANT_STAGED: u64 = 0x737461676564;
pub const PUBLISH_GRANT: u64 = 0x7075626c697368;
pub fn finish() -> Bench<()> {
    for expected in [READY, STOP] {
        let (token, word) = ipc::receive().map_err(|_| 84)?;
        if word != expected {
            return Err(84);
        }
        ipc::reply(token, word).map_err(|_| 84)?;
    }
    Ok(())
}
pub fn state() -> Bench<dev::State> {
    dev::info().map_err(|_| 82)
}

#[derive(Clone, Copy)]
pub struct Timeline {
    pub epoch: u64,
    pub hz: u64,
}
impl Timeline {
    pub fn new(epoch: u64, hz: u64) -> Bench<Self> {
        if hz == 0 || hz % 1000 != 0 {
            return Err(83);
        }
        let t = Self { epoch, hz };
        t.at(120)?;
        Ok(t)
    }
    pub fn at(self, ms: u64) -> Bench<u64> {
        (self.hz / 1000)
            .checked_mul(ms)
            .and_then(|t| self.epoch.checked_add(t))
            .ok_or(83)
    }
    pub fn start(case: Case) -> Bench<Self> {
        let (token, epoch) = ipc::receive().map_err(|_| 84)?;
        let now = state()?;
        let t = Self::new(epoch, now.hz)?;
        ipc::reply(token, ACK | case as u64).map_err(|_| 84)?;
        Ok(t)
    }
    // CLOCK_MONOTONIC counts from the kernel boot epoch. SVC 61 supplies the raw
    // counter. Convert only their short remaining interval, waking from a real
    // blocking sleep and resampling. Never treat a scheduled tick as observed.
    pub fn until(self, target: u64) -> Bench<u64> {
        loop {
            let mono = monotonic_time().map_err(|_| 85)?;
            let raw = state()?;
            if raw.hz != self.hz {
                return Err(83);
            }
            if raw.ticks >= target {
                return Ok(raw.ticks);
            }
            let ns = (u128::from(target - raw.ticks) * 1_000_000_000).div_ceil(u128::from(self.hz));
            let delay = Duration::from_nanos(u64::try_from(ns).map_err(|_| 83)?);
            sleep_until(mono.checked_add(delay).ok_or(83)?).map_err(|_| 85)?;
        }
    }
    pub fn wait(self, ms: u64) -> Bench<u64> {
        self.until(self.at(ms)?)
    }
}
pub fn status<T>(r: Result<T, i64>) -> i64 {
    r.err().unwrap_or(0)
}
