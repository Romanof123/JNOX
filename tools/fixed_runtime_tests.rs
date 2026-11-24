use crate::fixed_policy::{Action, CompareOverflow, Config, Fixed, Mode};

fn config(priority: u8, delay: u64) -> Config {
    Config { priority, delay }
}

#[test]
fn empty_invalid_and_counter_horizon() {
    let empty = Fixed::new(&[], Mode::Fifo, u64::MAX).unwrap();
    assert!(empty.finished());
    assert_eq!(empty.deadline().unwrap(), None);
    assert_eq!(empty.service(0), None);
    let c = config(255, 0);
    assert!(Fixed::new(&[c; 5], Mode::Fifo, 0).is_none());
    assert!(Fixed::new(&[], Mode::RoundRobin(0), 0).is_none());
    assert!(Fixed::new(&[config(0, 2)], Mode::Fifo, u64::MAX - 1).is_none());
    assert!(Fixed::new(&[c; 2], Mode::RoundRobin(4), u64::MAX - 2).is_none());
    let mut rr = Fixed::new(&[c; 2], Mode::RoundRobin(2), u64::MAX - 2).unwrap();
    assert_eq!(rr.deadline(), Ok(Some(u64::MAX)));
    assert_eq!(rr.event(u64::MAX, Action::Poll), Some(true));
    assert_eq!(rr.deadline(), Err(CompareOverflow));
    let mut p = Fixed::new(&[c; 2], Mode::Fifo, u64::MAX - 2).unwrap();
    assert_eq!(p.deadline().unwrap(), None);
    assert_eq!(p.event(u64::MAX - 1, Action::Sleep(2)), Some(false));
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.event(u64::MAX - 2, Action::Exit), None);
    assert_eq!(p.service(0), Some(1));
    assert_eq!(p.event(u64::MAX - 1, Action::Sleep(1)), Some(true));
    assert_eq!(p.deadline().unwrap(), Some(u64::MAX));
    assert_eq!(p.event(u64::MAX, Action::Poll), Some(true));
    assert_eq!(p.current(), Some(1));
}

#[test]
fn fifo_preemption_preserves_turn_and_svc_owns_original_frame() {
    let mut p = Fixed::new(
        &[config(32, 0), config(32, 0), config(33, 5)],
        Mode::Fifo,
        11,
    )
    .unwrap();
    assert_eq!(p.deadline().unwrap(), Some(16));
    p.event(17, Action::Poll).unwrap();
    assert_eq!(p.current(), Some(2));
    assert_eq!(p.service(0), Some(6));
    p.event(18, Action::Sleep(4)).unwrap();
    assert_eq!(p.current(), Some(0));
    p.event(22, Action::Exit).unwrap();
    assert_eq!(p.current(), Some(2));
    p.event(22, Action::Exit).unwrap();
    assert_eq!(p.current(), Some(1));
    assert_eq!(p.service(0), Some(10));
    assert_eq!(p.deadline().unwrap(), None);
}

#[test]
fn idle_and_overdue_wakes_use_deadline_then_slot() {
    let mut p = Fixed::new(&[config(8, 5), config(8, 4), config(8, 4)], Mode::Fifo, 100).unwrap();
    assert_eq!(p.current(), None);
    assert_eq!(p.deadline().unwrap(), Some(104));
    assert_eq!(p.event(150, Action::Yield), None);
    p.event(109, Action::Poll).unwrap();
    for id in [1, 2, 0] {
        assert_eq!(p.current(), Some(id));
        p.event(109, Action::Exit).unwrap();
    }
    assert!(p.finished());
    assert_eq!(p.deadline().unwrap(), None);
}

#[test]
fn residual_rr_survives_preemption_then_rotates_once_on_late_irq() {
    let mut p = Fixed::new(
        &[config(5, 0), config(5, 0), config(255, 3)],
        Mode::RoundRobin(8),
        0,
    )
    .unwrap();
    assert_eq!(p.deadline().unwrap(), Some(3));
    p.event(3, Action::Poll).unwrap();
    assert_eq!(p.current(), Some(2));
    assert_eq!(p.deadline().unwrap(), None);
    p.event(16, Action::Exit).unwrap();
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.deadline().unwrap(), Some(21));
    p.event(50, Action::Poll).unwrap();
    assert_eq!(p.current(), Some(1));
    assert_eq!(p.service(0), Some(37));
    assert_eq!(p.service(2), Some(13));
    assert_eq!(p.deadline().unwrap(), Some(58));
    p.event(51, Action::Sleep(0)).unwrap();
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.deadline().unwrap(), Some(59));
}

#[test]
fn uncontested_service_exhausts_slice_and_due_peers_precede_rotation() {
    let mut p = Fixed::new(
        &[config(1, 0), config(1, 9), config(2, 9), config(1, 9)],
        Mode::RoundRobin(3),
        10,
    )
    .unwrap();
    assert_eq!(p.deadline().unwrap(), Some(19));
    p.event(14, Action::Poll).unwrap();
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.deadline().unwrap(), Some(19));
    p.event(19, Action::Poll).unwrap();
    assert_eq!(p.current(), Some(2));
    p.event(20, Action::Exit).unwrap();
    assert_eq!(p.current(), Some(1));
    p.event(20, Action::Yield).unwrap();
    assert_eq!(p.current(), Some(3));
    p.event(20, Action::Yield).unwrap();
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.deadline().unwrap(), Some(23));
}

#[test]
fn relative_intervals_fit_the_signed_compare_distance() {
    let limit = i64::MAX as u64;
    let c = config(12, 0);
    assert!(Fixed::new(&[c; 2], Mode::RoundRobin(limit + 1), 0).is_none());
    assert!(Fixed::new(&[config(12, limit + 1)], Mode::Fifo, 0).is_none());
    let mut p = Fixed::new(&[c; 2], Mode::RoundRobin(limit), 0).unwrap();
    assert_eq!(p.deadline(), Ok(Some(limit)));
    assert_eq!(p.event(0, Action::Sleep(limit + 1)), Some(false));
    assert_eq!(p.current(), Some(0));
    assert_eq!(p.event(0, Action::Sleep(limit)), Some(true));
    assert_eq!(p.deadline(), Ok(Some(limit)));
}
