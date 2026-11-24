//! Host oracle for energy placement under per-CPU limits and protected deadlines.
use crate::group_policy::{EnergyRequest, OperatingPoint};
use crate::power_policy::{CpuLimits, Decision, Placement, place};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record {
    cpu: u8,
    opp: u16,
    capacity: u64,
    run_ticks: u64,
    wake_ticks: u64,
    margin: u64,
    energy: u128,
}

impl From<Placement> for Record {
    fn from(p: Placement) -> Self {
        Self {
            cpu: p.cpu,
            opp: p.opp,
            capacity: p.capacity,
            run_ticks: p.run_ticks,
            wake_ticks: p.wake_ticks,
            margin: p.margin,
            energy: p.energy,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Answer {
    Model(Record),
    Fallback(Record),
    Idle,
    DeadlineMiss,
}

impl From<Decision> for Answer {
    fn from(d: Decision) -> Self {
        match d {
            Decision::Model(p) => Self::Model(p.into()),
            Decision::Fallback(p) => Self::Fallback(p.into()),
            Decision::Idle => Self::Idle,
            Decision::DeadlineMiss => Self::DeadlineMiss,
        }
    }
}

fn point(
    cpu: u8,
    opp: u16,
    capacity: u64,
    energy_per_work: u64,
    wake_energy: u64,
    wake_ticks: u64,
) -> OperatingPoint {
    OperatingPoint {
        cpu,
        opp,
        capacity,
        energy_per_work,
        wake_energy,
        wake_ticks,
    }
}

fn limits(
    available: bool,
    thermal_capacity: u64,
    shared_load: u64,
    reserve_capacity: u64,
) -> CpuLimits {
    CpuLimits {
        available,
        thermal_capacity,
        shared_load,
        reserve_capacity,
    }
}

fn request(
    allowed_cpus: u64,
    work: u64,
    minimum_capacity: u64,
    now: u64,
    deadline: u64,
) -> EnergyRequest {
    EnergyRequest {
        allowed_cpus,
        work,
        minimum_capacity,
        now,
        deadline,
    }
}

// Small cases enumerate the available work lanes, then consume work and energy
// one unit at a time. A capacity-qualified late candidate affects the verdict
// even when an independent fallback has no access to a usable CPU.
fn small_answer<const N: usize, const C: usize>(
    r: EnergyRequest,
    reserve: u64,
    model: &[OperatingPoint; N],
    cpus: &[CpuLimits; C],
    fallback: Option<OperatingPoint>,
) -> Answer {
    if r.work == 0 {
        return Answer::Idle;
    }
    let mut missed = false;
    let mut simulate = |p: &OperatingPoint| -> Option<Record> {
        let cpu = cpus.get(usize::from(p.cpu))?;
        if !cpu.available || (r.allowed_cpus & 1u64.checked_shl(u32::from(p.cpu))?) == 0 {
            return None;
        }
        let occupied = u128::from(cpu.shared_load) + u128::from(cpu.reserve_capacity);
        let lanes = (0..p.capacity)
            .filter(|&lane| lane < cpu.thermal_capacity && u128::from(lane) >= occupied)
            .count() as u64;
        if lanes == 0 || lanes < r.minimum_capacity {
            return None;
        }
        let mut remaining = r.work;
        let mut run = 0;
        let mut energy = u128::from(p.wake_energy);
        while remaining > 0 {
            run += 1;
            for _ in 0..lanes.min(remaining) {
                energy += u128::from(p.energy_per_work);
                remaining -= 1;
            }
        }
        let finish = u128::from(r.now) + u128::from(p.wake_ticks) + u128::from(run);
        if finish + u128::from(reserve) > u128::from(r.deadline) {
            missed = true;
            return None;
        }
        Some(Record {
            cpu: p.cpu,
            opp: p.opp,
            capacity: lanes,
            run_ticks: run,
            wake_ticks: p.wake_ticks,
            margin: r.deadline - finish as u64,
            energy,
        })
    };
    let mut feasible: Vec<_> = model.iter().filter_map(&mut simulate).collect();
    feasible.sort_by_key(|p| (p.energy, p.cpu, p.opp));
    if let Some(&best) = feasible.first() {
        return Answer::Model(best);
    }
    if let Some(p) = fallback.and_then(|p| simulate(&p)) {
        return Answer::Fallback(p);
    }
    if missed {
        Answer::DeadlineMiss
    } else {
        Answer::Idle
    }
}

#[test]
fn small_placements_match_work_lane_and_tick_enumeration() {
    let points = [
        point(1, 9, 5, 1, 1, 2),
        point(0, 7, 2, 3, 0, 0),
        point(0, 2, 4, 2, 2, 1),
    ];
    let fallback = point(1, 1, 6, 0, 0, 0);
    let loads = [(5, 0, 0), (3, 1, 0), (5, 1, 2), (1, 2, 1), (0, 0, 0)];
    for mask in 0..4 {
        for online in 0..4 {
            for i in 0..loads.len() {
                let cpus = std::array::from_fn::<_, 2, _>(|cpu| {
                    let (thermal, shared, held) = loads[(i + 2 * cpu) % loads.len()];
                    limits(online & (1 << cpu) != 0, thermal, shared, held)
                });
                for work in [0, 1, 3, 7] {
                    for minimum in [0, 2, 4] {
                        for window in 0..=6 {
                            let r = request(mask, work, minimum, 11, 11 + window);
                            for reserve in [0, 2] {
                                for fb in [None, Some(fallback)] {
                                    let actual = place(r, reserve, &points, &cpus, fb);
                                    assert_eq!(
                                        Answer::from(actual),
                                        small_answer(r, reserve, &points, &cpus, fb),
                                        "affinity {mask}, online {online}, loads {i}, work {work}, minimum {minimum}, window {window}, reserve {reserve}, fallback {}",
                                        fb.is_some()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn equal_energy_is_ordered_by_cpu_then_opp_even_with_a_cheaper_fallback() {
    let points = [
        point(1, 0, 4, 1, 2, 1),
        point(0, 9, 4, 2, 0, 0),
        point(0, 3, 2, 0, 4, 0),
    ];
    let cpus = [limits(true, 4, 0, 0), limits(true, 4, 0, 0)];
    let r = request(3, 2, 1, 5, 10);
    let result = Answer::Model(Record {
        cpu: 0,
        opp: 3,
        capacity: 2,
        run_ticks: 1,
        wake_ticks: 0,
        margin: 4,
        energy: 4,
    });
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let table = order.map(|i| points[i]);
        assert_eq!(
            Answer::from(place(r, 1, &table, &cpus, Some(point(0, 1, 3, 0, 0, 0)))),
            result
        );
    }
}

#[test]
fn duplicate_keys_invalidate_the_whole_model_before_cpu_filtering() {
    let valid = point(0, 5, 3, 1, 0, 0);
    let duplicate = point(1, 7, 2, 1, 0, 0);
    let other = point(1, 7, 4, 5, 9, 3);
    let cpus = [limits(true, 5, 0, 0), limits(false, 4, 0, 0)];
    let r = request(1, 3, 1, 10, 17);
    let fb = point(0, 5, 2, 9, 1, 1); // also a key in the invalid model
    for table in [
        [valid, duplicate, duplicate],
        [other, valid, duplicate],
        [duplicate, other, valid],
    ] {
        assert_eq!(Answer::from(place(r, 0, &table, &cpus, None)), Answer::Idle);
        let expected = Record {
            cpu: 0,
            opp: 5,
            capacity: 2,
            run_ticks: 2,
            wake_ticks: 1,
            margin: 4,
            energy: 28,
        };
        assert_eq!(
            Answer::from(place(r, 4, &table, &cpus, Some(fb))),
            Answer::Fallback(expected)
        );
        assert_eq!(
            Answer::from(place(r, 5, &table, &cpus, Some(fb))),
            Answer::DeadlineMiss
        );
    }
}

#[test]
fn empty_inputs_unusable_capacity_and_late_eligible_routes_have_distinct_outcomes() {
    let p = point(0, 0, 5, 1, 0, 1);
    let cpus = [limits(true, 3, 1, 1)]; // one effective work unit
    let past = request(1, 2, 1, 11, 10);
    assert_eq!(Answer::from(place(past, 0, &[], &cpus, None)), Answer::Idle);
    assert_eq!(
        Answer::from(place(past, 0, &[p], &[], Some(p))),
        Answer::Idle
    );
    assert_eq!(
        Answer::from(place(past, 0, &[p], &cpus, None)),
        Answer::DeadlineMiss
    );
    assert_eq!(
        Answer::from(place(past, 0, &[], &cpus, Some(p))),
        Answer::DeadlineMiss
    );
    assert_eq!(
        Answer::from(place(request(1, 0, 1, 11, 10), 0, &[p], &cpus, Some(p))),
        Answer::Idle
    );
    assert_eq!(
        Answer::from(place(request(1, 2, 2, 11, 10), 0, &[p], &cpus, Some(p))),
        Answer::Idle
    );
    let fit = request(1, 2, 1, 11, 16);
    let exact = Record {
        cpu: 0,
        opp: 0,
        capacity: 1,
        run_ticks: 2,
        wake_ticks: 1,
        margin: 2,
        energy: 2,
    };
    assert_eq!(
        Answer::from(place(fit, 2, &[], &cpus, Some(p))),
        Answer::Fallback(exact)
    );
    assert_eq!(
        Answer::from(place(fit, 3, &[p], &cpus, None)),
        Answer::DeadlineMiss
    );
    let faster = point(0, 2, 4, 6, 1, 0);
    let fast = Record {
        cpu: 0,
        opp: 2,
        capacity: 1,
        run_ticks: 2,
        wake_ticks: 0,
        margin: 3,
        energy: 13,
    };
    assert_eq!(
        Answer::from(place(fit, 3, &[p], &cpus, Some(faster))),
        Answer::Fallback(fast)
    );
}

#[test]
fn arithmetic_horizon_and_protected_margin_use_full_width_values() {
    let m = u64::MAX;
    let p = point(0, u16::MAX, m - 1, m, m, 1);
    let cpus = [limits(true, m, 0, 0)];
    let r = request(1, m, m - 1, m - 3, m);
    let exact = Record {
        cpu: 0,
        opp: u16::MAX,
        capacity: m - 1,
        run_ticks: 2,
        wake_ticks: 1,
        margin: 0,
        energy: u128::from(m) * (u128::from(m) + 1),
    };
    assert_eq!(
        Answer::from(place(r, 0, &[p], &cpus, None)),
        Answer::Model(exact)
    );
    assert_eq!(
        Answer::from(place(r, 1, &[p], &cpus, None)),
        Answer::DeadlineMiss
    );
    assert_eq!(
        Answer::from(place(request(1, m, 1, m - 2, m), 0, &[p], &cpus, None)),
        Answer::DeadlineMiss
    );
    assert_eq!(
        Answer::from(place(request(1, 1, 1, 0, 1), m, &[p], &cpus, None)),
        Answer::DeadlineMiss
    );
    let wake_wrap = point(0, 2, 1, 0, 0, m);
    assert_eq!(
        Answer::from(place(
            request(1, 1, 1, 1, m),
            0,
            &[],
            &cpus,
            Some(wake_wrap)
        )),
        Answer::DeadlineMiss
    );
    let saturated = [limits(true, m, m - 1, 2)];
    assert_eq!(
        Answer::from(place(r, 0, &[p], &saturated, Some(p))),
        Answer::Idle
    );
    let immediate = point(0, 0, 1, 0, 0, 0);
    let wide = request(1, 1, 1, 0, m);
    let slack = Record {
        cpu: 0,
        opp: 0,
        capacity: 1,
        run_ticks: 1,
        wake_ticks: 0,
        margin: m - 1,
        energy: 0,
    };
    assert_eq!(
        Answer::from(place(wide, m - 1, &[immediate], &cpus, None)),
        Answer::Model(slack)
    );
    assert_eq!(
        Answer::from(place(wide, m, &[immediate], &cpus, None)),
        Answer::DeadlineMiss
    );
}

#[test]
fn energy_comparison_preserves_upper_product_bits() {
    let m = u64::MAX;
    let costly = point(0, 0, m, 2, 2, 0); // exactly 2^65
    let cheaper = point(0, 1, m, 1, 1, 0); // exactly 2^64
    let r = request(1, m, 1, 0, 1);
    let cpus = [limits(true, m, 0, 0)];
    let selected = Record {
        cpu: 0,
        opp: 1,
        capacity: m,
        run_ticks: 1,
        wake_ticks: 0,
        margin: 0,
        energy: 1u128 << 64,
    };
    assert_eq!(
        Answer::from(place(r, 0, &[costly, cheaper], &cpus, None)),
        Answer::Model(selected)
    );
}

#[test]
fn highest_mask_bit_is_usable_and_core_indices_outside_it_are_ineligible() {
    let cpus: [CpuLimits; 65] = std::array::from_fn(|_| limits(true, 2, 0, 0));
    let upper = point(63, 6, 2, 3, 0, 0);
    let outside = point(64, 0, 2, 0, 0, 0);
    let r = request(1u64 << 63, 2, 1, 9, 10);
    let expected = Record {
        cpu: 63,
        opp: 6,
        capacity: 2,
        run_ticks: 1,
        wake_ticks: 0,
        margin: 0,
        energy: 6,
    };
    assert_eq!(
        Answer::from(place(r, 0, &[outside, upper], &cpus, None)),
        Answer::Model(expected)
    );
    assert_eq!(
        Answer::from(place(r, 0, &[outside], &cpus, Some(outside))),
        Answer::Idle
    );
}
