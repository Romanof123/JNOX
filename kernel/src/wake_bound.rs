//! Closed, four-slot profile. All arguments use architectural counter counts.
pub fn bound(unit: u64, slots: usize) -> Option<u64> {
    if unit == 0 || slots != 4 { return None; }
    let period = unit.checked_mul(10)?;
    let peer = unit.checked_mul(2)?.checked_add(unit)?;
    period.checked_add(unit)?
        .checked_add(peer.checked_mul((slots - 1) as u64)?)?
        .checked_add(unit)?
        .checked_add(unit.checked_mul(2)?)
}

pub fn admits(unit: u64, slots: usize, limit: u64) -> bool {
    bound(unit, slots).is_some_and(|b| b <= limit)
}
