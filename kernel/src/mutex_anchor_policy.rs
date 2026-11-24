//! One successful LOW acquisition can designate one subsequent return in a boot.
pub(crate) fn exact_ticks(frequency: u64) -> Option<u64> {
    (frequency != 0 && frequency % 10_000 == 0).then_some(frequency / 10_000)
}

pub(crate) fn valid_identity(frame: u64, pc: u64, spsr: u64, tls: u64, x0: u64) -> bool {
    frame != 0
        && frame % 16 == 0
        && pc != 0
        && pc % 4 == 0
        && spsr & 0x8f == 5
        && tls == 1
        && x0 == 0
}

pub(crate) fn valid_readback(target: u64, cval: u64, ctl: u64) -> bool {
    target == cval && ctl & 3 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_clock_and_readback() {
        assert_eq!(exact_ticks(62_500_000), Some(6250));
        assert_eq!(exact_ticks(0), None);
        assert_eq!(exact_ticks(62_500_001), None);
        assert!(valid_readback(30, 30, 1));
        assert!(valid_readback(30, 30, 5));
        assert!(!valid_readback(30, 31, 1));
        assert!(!valid_readback(30, 30, 3));
    }
    #[test]
    fn selected_low_return() {
        assert!(valid_identity(0x1000, 0x2000, 0x345, 1, 0));
        assert!(!valid_identity(0x1008, 0x2000, 0x345, 1, 0));
        assert!(!valid_identity(0x1000, 0x2000, 0x3c5, 1, 0));
        assert!(!valid_identity(0x1000, 0x2000, 0x345, 2, 0));
        assert!(!valid_identity(0x1000, 0x2000, 0x345, 1, 1));
    }
}
