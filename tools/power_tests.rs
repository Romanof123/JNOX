#![allow(dead_code)]
#[path = "../kernel/src/group_policy.rs"]
mod group_policy;
#[path = "../kernel/src/power_policy.rs"]
mod power_policy;
use group_policy::{EnergyRequest, OperatingPoint};
use power_policy::{CpuLimits, Decision, Placement, place};

const SLOW: OperatingPoint = OperatingPoint {
    cpu: 0,
    opp: 0,
    capacity: 3,
    energy_per_work: 1,
    wake_ticks: 1,
    wake_energy: 1,
};
const FAST: OperatingPoint = OperatingPoint {
    cpu: 1,
    opp: 1,
    capacity: 6,
    energy_per_work: 3,
    wake_ticks: 1,
    wake_energy: 2,
};
const FREE: CpuLimits = CpuLimits {
    available: true,
    thermal_capacity: u64::MAX,
    shared_load: 0,
    reserve_capacity: 0,
};
const WORK: EnergyRequest = EnergyRequest {
    allowed_cpus: 3,
    work: 12,
    minimum_capacity: 1,
    now: 0,
    deadline: 6,
};

fn fast() -> Placement {
    Placement {
        cpu: 1,
        opp: 1,
        capacity: 6,
        run_ticks: 2,
        wake_ticks: 1,
        margin: 3,
        energy: 38,
    }
}

#[test]
fn same_workload_across_catalogue_and_constraints() {
    let points = [FAST, SLOW];
    assert_eq!(
        place(WORK, 1, &points, &[FREE; 2], Some(FAST)),
        Decision::Model(Placement {
            cpu: 0,
            opp: 0,
            capacity: 3,
            run_ticks: 4,
            wake_ticks: 1,
            margin: 1,
            energy: 13
        })
    );
    for limit in [
        CpuLimits {
            thermal_capacity: 2,
            ..FREE
        },
        CpuLimits {
            shared_load: 1,
            ..FREE
        },
        CpuLimits {
            reserve_capacity: 1,
            ..FREE
        },
        CpuLimits {
            available: false,
            ..FREE
        },
    ] {
        assert_eq!(
            place(WORK, 1, &points, &[limit, FREE], None),
            Decision::Model(fast())
        );
    }
    assert_eq!(
        place(WORK, 1, &[], &[FREE; 2], Some(FAST)),
        Decision::Fallback(fast())
    );
    assert_eq!(
        place(WORK, 1, &[SLOW, SLOW], &[FREE; 2], Some(FAST)),
        Decision::Fallback(fast())
    );
    assert_eq!(
        place(
            WORK,
            1,
            &points,
            &[CpuLimits {
                thermal_capacity: 1,
                ..FREE
            }; 2],
            None
        ),
        Decision::DeadlineMiss
    );
    assert_eq!(
        place(
            EnergyRequest {
                allowed_cpus: 0,
                ..WORK
            },
            1,
            &points,
            &[FREE; 2],
            Some(FAST)
        ),
        Decision::Idle
    );
}

#[test]
fn trusted_fallback_obeys_constraints_and_overflow_bounds() {
    assert_eq!(
        place(WORK, 7, &[], &[FREE; 2], Some(FAST)),
        Decision::DeadlineMiss
    );
    assert_eq!(place(WORK, 1, &[], &[FREE], Some(FAST)), Decision::Idle);
    assert_eq!(
        place(
            WORK,
            1,
            &[],
            &[CpuLimits {
                shared_load: u64::MAX,
                reserve_capacity: u64::MAX,
                ..FREE
            }; 2],
            Some(FAST)
        ),
        Decision::Idle
    );
    assert_eq!(
        place(
            EnergyRequest {
                now: u64::MAX,
                deadline: u64::MAX,
                ..WORK
            },
            0,
            &[],
            &[FREE; 2],
            Some(FAST)
        ),
        Decision::DeadlineMiss
    );
    let huge = OperatingPoint {
        capacity: u64::MAX,
        energy_per_work: u64::MAX,
        wake_energy: u64::MAX,
        wake_ticks: 0,
        ..FAST
    };
    let request = EnergyRequest {
        work: u64::MAX,
        deadline: 1,
        ..WORK
    };
    let Decision::Model(placement) = place(request, 0, &[huge], &[FREE; 2], None) else {
        panic!()
    };
    assert_eq!(placement.energy, u128::from(u64::MAX) << 64);
    assert_eq!(placement.margin, 0);
}
