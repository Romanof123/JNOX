//! Tests of the single-CPU policy exercised by the proportional guest.

use crate::proportional_policy::Proportional;

const WEIGHTS: [u8; 3] = [1, 2, 4];

#[test]
fn every_active_ticket_interval_and_its_endpoints() {
    let mut policy = Proportional::lottery(WEIGHTS, 0).unwrap();
    for mask in 0..8 {
        assert!(policy.set_ready_mask(mask));
        let mut expected = Vec::new();
        for (id, &weight) in WEIGHTS.iter().enumerate() {
            if mask & (1 << id) != 0 {
                expected.extend(core::iter::repeat_n(id, usize::from(weight)));
            }
        }
        for (draw, &id) in expected.iter().enumerate() {
            assert_eq!(policy.winner_for_draw(draw as u32), Some(id));
        }
        assert_eq!(policy.winner_for_draw(expected.len() as u32), None);
        assert_eq!(policy.winner_for_draw(u32::MAX), None);
        assert_eq!(policy.last_draw(), None);
    }
}

#[test]
fn forced_rejection_and_replay_use_the_released_draw_path() {
    // Seed = -gamma; SplitMix64's first raw word is zero. For seven tickets
    // the rejected raw values are 0 and 1; the next raw word is the published
    // first output of SplitMix64 seeded with zero.
    let mut forced = Proportional::lottery(WEIGHTS, 0x61c8_8646_80b5_83eb).unwrap();
    let mut direct = Proportional::lottery(WEIGHTS, 0).unwrap();
    assert_eq!(forced.dispatch(), None);
    assert!(forced.set_ready_mask(7));
    assert!(direct.set_ready_mask(7));
    for turn in 0..128 {
        let winner = forced.dispatch().unwrap();
        assert_eq!(direct.dispatch(), Some(winner));
        assert_eq!(forced.last_draw(), direct.last_draw());
        if turn == 0 {
            assert_eq!(forced.last_draw(), Some(((0xe220_a839_7b1d_cdafu64 % 7) as u32, 7)));
        }
        assert_eq!(forced.dispatch(), None);
        assert!(!forced.set_ready_mask(1));
        assert!(!forced.stop((winner + 1) % 3, 7));
        assert!(forced.stop(winner, 1));
        assert!(!forced.stop(winner, 1));
        assert!(direct.stop(winner, 1));
    }
}

fn lottery_counts(seed: u64, draws: u32) -> [u32; 3] {
    let mut policy = Proportional::lottery(WEIGHTS, seed).unwrap();
    assert!(policy.set_ready_mask(7));
    let mut counts = [0; 3];
    for _ in 0..draws {
        let id = policy.dispatch().unwrap();
        let (draw, total) = policy.last_draw().unwrap();
        assert_eq!(total, 7);
        assert_eq!(policy.winner_for_draw(draw), Some(id));
        counts[id] += 1;
        assert!(policy.stop(id, 1));
    }
    counts
}

#[test]
fn lottery_finite_seed_windows_use_predeclared_integer_tolerances() {
    for seed in 1..=8 {
        let window = 4096;
        for (count, weight) in lottery_counts(seed, window).into_iter().zip(WEIGHTS) {
            // Absolute share error <= 3 percentage points, for every client
            // at every host seed separately.
            let error = (7 * count).abs_diff(u32::from(weight) * window);
            assert!(100 * error <= 3 * 7 * window, "host seed {seed}, weight {weight}, count {count}");
        }
    }
    for seed in 1..=2 {
        let window = 32;
        for (count, weight) in lottery_counts(seed, window).into_iter().zip(WEIGHTS) {
            let error = (7 * count).abs_diff(u32::from(weight) * window);
            assert!(error <= 7 * 8, "guest seed {seed}, weight {weight}, count {count}");
        }
    }
}

#[test]
fn stride_delivers_weighted_service_with_unequal_dispatch_durations() {
    let mut policy = Proportional::stride(WEIGHTS).unwrap();
    assert!(policy.set_ready_mask(7));
    let mut service = [0; 3];
    let mut dispatches = [0; 3];
    for _ in 0..72 {
        let id = policy.dispatch().unwrap();
        let duration = [2, 1, 2][id];
        service[id] += duration;
        dispatches[id] += 1;
        assert!(policy.stop(id, duration));
    }
    assert_eq!(service, [16, 32, 64]);
    assert_eq!(dispatches, [8, 32, 32]);
}

#[test]
fn stride_late_join_uses_the_active_floor_and_normalization_preserves_order() {
    let mut policy = Proportional::stride(WEIGHTS).unwrap();
    assert!(policy.set_ready_mask(3));
    assert_eq!(policy.dispatch(), Some(0));
    assert!(policy.stop(0, 2));
    assert_eq!(policy.dispatch(), Some(1));
    assert!(policy.stop(1, 1));
    assert!(policy.set_ready_mask(7));
    assert_eq!(policy.pass(2), Some(360_360));
    assert_eq!(policy.pass(0), Some(1_441_440));
    assert_eq!(policy.pass(1), Some(360_360));
    assert_eq!(policy.dispatch(), Some(1)); // ties with the new task, lower ID wins
    assert_eq!(policy.last_normalization(), Some(360_360));
    assert_eq!(policy.pass(0), Some(1_081_080));
    assert_eq!(policy.pass(1), Some(0));
    assert_eq!(policy.pass(2), Some(0));
    assert!(policy.stop(1, 1));
    assert_eq!(policy.dispatch(), Some(2));
}

#[test]
fn stride_debt_survives_short_sleep_large_charges_and_idle() {
    let mut policy = Proportional::stride([1, 16]).unwrap();
    assert!(policy.set_ready_mask(3));
    assert_eq!(policy.dispatch(), Some(0));
    assert!(policy.stop(0, u32::MAX));
    let debt = u64::from(u32::MAX) * 720_720;
    assert_eq!(policy.pass(0), Some(debt));
    assert!(policy.set_ready_mask(2));
    assert_eq!(policy.dispatch(), Some(1));
    assert!(policy.stop(1, 1));
    assert!(policy.set_ready_mask(0));
    assert_eq!(policy.dispatch(), None);
    assert!(policy.set_ready_mask(3));
    assert_eq!(policy.pass(0), Some(debt - 45_045));
    assert_eq!(policy.dispatch(), Some(1));
    assert!(policy.stop(1, 0));
    assert_eq!(policy.pass(1), Some(0));
}

#[test]
fn configuration_and_mask_bounds_include_the_last_bit() {
    assert!(Proportional::<0>::lottery([], 0).is_none());
    assert!(Proportional::<33>::stride([1; 33]).is_none());
    assert!(Proportional::stride([0]).is_none());
    assert!(Proportional::lottery([17], 2).is_none());
    let mut small = Proportional::stride(WEIGHTS).unwrap();
    assert!(small.set_ready_mask(2));
    assert!(!small.set_ready_mask(8));
    assert_eq!(small.ready_mask(), 2);
    assert_eq!(small.pass(3), None);
    let mut full = Proportional::<32>::lottery([16; 32], 1).unwrap();
    assert!(full.set_ready_mask(u32::MAX));
    assert_eq!(full.winner_for_draw(511), Some(31));
    assert_eq!(full.winner_for_draw(512), None);
    assert!(full.set_ready_mask(1 << 31));
    assert_eq!(full.dispatch(), Some(31));
}

#[test]
fn supplied_words_do_not_advance_an_autonomous_draw() {
    use crate::proportional_policy::reduce_word;
    assert_eq!(reduce_word(0, 7), None);
    assert_eq!(reduce_word(1, 7), None);
    assert_eq!(reduce_word(2, 7), Some(2));
    assert_eq!(reduce_word(7, 7), Some(0));
    assert_eq!(reduce_word(6, 7), Some(6));
    assert_eq!(reduce_word(2, 0), None);
}

#[test]
fn finite_windows_cover_edges_and_reject_unsupported_contracts() {
    use crate::proportional_policy::WindowInput;
    let input = WindowInput {
        seed: 1, weights: WEIGHTS, ready: 7, horizon: 48, width: 24, floors: [5, 3, 11],
    };
    let admitted = input.analyze().unwrap();
    assert_eq!(admitted.minimum, input.floors);
    assert_eq!(admitted.first_short, None);
    let tight = WindowInput { floors: [6, 3, 11], ..input }.analyze().unwrap();
    assert_eq!(tight.minimum, admitted.minimum);
    assert_eq!(tight.first_short, Some((24, 0, 5)));
    let seed_two = WindowInput { seed: 2, floors: [1, 7, 10], ..input };
    assert_eq!(seed_two.analyze().unwrap().minimum, seed_two.floors);
    assert_eq!(seed_two.analyze().unwrap().first_short, None);
    let whole = WindowInput { width: 48, floors: [11, 14, 23], ..input };
    assert_eq!(whole.analyze().unwrap().minimum, whole.floors);
    for invalid in [
        WindowInput { horizon: 0, ..input },
        WindowInput { horizon: 65, ..input },
        WindowInput { width: 0, ..input },
        WindowInput { width: 49, ..input },
        WindowInput { width: 1, floors: [1; 3], ..input },
        WindowInput { weights: [0, 2, 4], ..input },
        WindowInput { weights: [17, 2, 4], ..input },
        WindowInput { ready: 3, ..input },
        WindowInput { ready: 15, ..input },
        WindowInput { floors: [0, 1, 1], ..input },
        WindowInput { floors: [25, 1, 1], ..input },
        WindowInput { floors: [1, 12, 12], ..input },
    ] {
        assert!(invalid.analyze().is_none());
    }
}
