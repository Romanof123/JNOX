use crate::schedule::{Result, Segment};
use jnox_native::safety_channel as dev;

#[cfg(not(feature = "ordinary"))]
pub use dev::{check_and_sample, complete_and_sample};

#[cfg(feature = "ordinary")]
pub fn check_and_sample() -> core::result::Result<dev::State, i64> {
    dev::check()?;
    dev::info()
}

#[cfg(feature = "ordinary")]
pub fn complete_and_sample() -> core::result::Result<dev::State, i64> {
    dev::complete()?;
    dev::info()
}

#[cfg(not(feature = "ordinary"))]
pub fn start(segment: &Segment, _hz: u64) -> Result<()> {
    segment.start()
}

#[cfg(feature = "ordinary")]
pub fn start(segment: &Segment, hz: u64) -> Result<()> {
    use core::time::Duration;
    use jnox_native::{monotonic_time, sleep_until};

    let before = crate::schedule::info()?;
    if hz == 0 || before.hz != hz {
        return Err(83);
    }
    if segment.target <= before.ticks {
        return Ok(());
    }
    // Sampling monotonic time after the raw counter includes their skew in
    // the measured delay. Each future release performs one absolute wait.
    let monotonic = monotonic_time().map_err(|_| 87)?;
    let delta = u128::from(segment.target - before.ticks) * 1_000_000_000;
    let ns = u64::try_from(delta.div_ceil(u128::from(hz))).map_err(|_| 83)?;
    let deadline = monotonic.checked_add(Duration::from_nanos(ns)).ok_or(83)?;
    sleep_until(deadline).map_err(|_| 87)?;
    let after = crate::schedule::info()?;
    if after.hz != hz || after.ticks < segment.target {
        return Err(87);
    }
    Ok(())
}

#[cfg(not(feature = "ordinary"))]
pub fn finish(segment: &Segment) -> Result<()> {
    segment.finish()
}

#[cfg(feature = "ordinary")]
pub fn finish(_segment: &Segment) -> Result<()> {
    Ok(())
}
