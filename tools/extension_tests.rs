use super::extension_policy::{Decision, Extension, MAX_FUEL, MAX_OPS, Op, Reason, Reservation};
use super::group_policy::{DelegatedTask, Proposal};

fn reservation() -> Reservation {
    Reservation {
        task: 2,
        period: 5,
        offset: 3,
        ticks: 1,
    }
}

fn tasks() -> [DelegatedTask; 3] {
    let task = DelegatedTask {
        generation: 7,
        ready: true,
        allowed_cpus: 0b10,
        grant_limit: 3,
    };
    [
        task,
        DelegatedTask {
            generation: 9,
            allowed_cpus: 0b11,
            ..task
        },
        DelegatedTask {
            generation: 12,
            ..task
        },
    ]
}

fn request() -> Proposal {
    Proposal {
        task: 1,
        generation: 9,
        cpu: 0,
        ticks: 2,
    }
}

fn fallback() -> Proposal {
    Proposal {
        task: 0,
        generation: 7,
        cpu: 1,
        ticks: 1,
    }
}

fn expected(reason: Reason, steps: u8, selected: Option<Proposal>) -> Decision {
    Decision {
        reason,
        steps,
        selected,
    }
}

#[test]
fn conditional_extension_consumes_only_visited_operations_and_gate_checks_all_fields() {
    let mut ext = Extension::new(reservation(), 10, 0, 20).unwrap();
    let mut jobs = tasks();
    let p = request();
    let code = [
        Op::IfReady {
            task: 1,
            yes: 2,
            no: 1,
        },
        Op::Yield,
        Op::Propose(p),
    ];
    assert_eq!(
        ext.decide(10, 0, &jobs, 3, 3, &code, 2),
        expected(Reason::Accepted, 2, Some(p))
    );
    jobs[1].ready = false;
    assert_eq!(
        ext.decide(10, 0, &jobs, 3, 3, &code, 2),
        expected(Reason::Yielded, 2, Some(fallback()))
    );
    assert_eq!(
        ext.decide(10, 0, &jobs, 3, 3, &[Op::Propose(p)], 1),
        expected(Reason::Rejected, 1, Some(fallback()))
    );
    jobs[1].ready = true;
    for bad in [
        Proposal {
            task: usize::MAX,
            ..p
        },
        Proposal { generation: 8, ..p },
        Proposal { cpu: 2, ..p },
        Proposal { cpu: 64, ..p },
        Proposal { ticks: 0, ..p },
        Proposal { ticks: 4, ..p },
        Proposal {
            task: 2,
            generation: 12,
            cpu: 1,
            ..p
        },
    ] {
        assert_eq!(
            ext.decide(10, 0, &jobs, 3, 3, &[Op::Propose(bad)], 1),
            expected(Reason::Rejected, 1, Some(fallback()))
        );
    }
    // CPU 0 is allowed by task 1 but excluded from the current CPU mask.
    assert_eq!(
        ext.decide(10, 0, &jobs, 2, 3, &[Op::Propose(p)], 1),
        expected(Reason::Rejected, 1, Some(fallback()))
    );
    jobs[1].grant_limit = 1;
    assert_eq!(
        ext.decide(10, 0, &jobs, 3, 3, &[Op::Propose(p)], 1),
        expected(Reason::Rejected, 1, Some(fallback()))
    );
}

#[test]
fn a_cycle_runs_at_most_the_supplied_fuel_and_the_final_step_can_still_submit() {
    let mut ext = Extension::new(reservation(), 0, 0, 10).unwrap();
    let jobs = tasks();
    let cycle = [Op::Jump(1), Op::Jump(0)];
    for fuel in 0..=MAX_FUEL {
        assert_eq!(
            ext.decide(0, 0, &jobs, 3, 3, &cycle, fuel),
            expected(Reason::Fuel, fuel, Some(fallback()))
        );
    }
    let mut chain = [Op::Yield; MAX_OPS];
    for (index, op) in chain.iter_mut().enumerate().take(MAX_OPS - 1) {
        *op = Op::Jump((index + 1) as u8);
    }
    chain[MAX_OPS - 1] = Op::Propose(request());
    assert_eq!(
        ext.decide(0, 0, &jobs, 3, 3, &chain, MAX_FUEL),
        expected(Reason::Accepted, MAX_FUEL, Some(request()))
    );
    assert_eq!(
        ext.decide(0, 0, &jobs, 3, 3, &chain, MAX_FUEL - 1),
        expected(Reason::Fuel, MAX_FUEL - 1, Some(fallback()))
    );
}

#[test]
fn malformed_or_absent_code_returns_without_executing_any_instruction() {
    let mut ext = Extension::new(reservation(), 0, 0, 10).unwrap();
    let jobs = tasks();
    for code in [
        &[Op::Jump(1)][..],
        &[Op::Yield, Op::Jump(2)][..], // Invalid even though instruction 1 is unreachable.
        &[Op::IfReady {
            task: 3,
            yes: 0,
            no: 0,
        }][..],
        &[Op::IfReady {
            task: 1,
            yes: 0,
            no: 1,
        }][..],
        &[Op::Yield; MAX_OPS + 1][..],
    ] {
        assert_eq!(
            ext.decide(0, 0, &jobs, 3, 3, code, 3),
            expected(Reason::InvalidProgram, 0, Some(fallback()))
        );
    }
    assert_eq!(
        ext.decide(0, 0, &jobs, 3, 3, &[Op::Yield], MAX_FUEL + 1),
        expected(Reason::InvalidProgram, 0, Some(fallback()))
    );
    assert_eq!(
        ext.decide(0, 0, &jobs, 3, 3, &[], 0),
        expected(Reason::Absent, 0, Some(fallback()))
    );
    assert_eq!(
        ext.decide(0, 0, &jobs, 0, 3, &[], 5),
        expected(Reason::Absent, 0, None)
    );
    assert_eq!(
        ext.decide(0, 0, &jobs, 3, 0, &[], 5),
        expected(Reason::Absent, 0, None)
    );
}

#[test]
fn watchdog_expires_on_the_sampled_deadline_and_only_forward_progress_rearms_it() {
    let mut ext = Extension::new(reservation(), 10, 100, 2).unwrap();
    let jobs = tasks();
    let p = Proposal {
        ticks: 1,
        ..request()
    };
    let code = [Op::Propose(p)];
    assert_eq!(
        ext.decide(11, 100, &jobs, 3, 3, &code, 1),
        expected(Reason::Accepted, 1, Some(p))
    );
    assert_eq!(
        ext.decide(12, 100, &jobs, 3, 3, &code, 1),
        expected(Reason::Stalled, 0, Some(fallback()))
    );
    let reserved = Proposal {
        task: 2,
        generation: 12,
        cpu: 1,
        ticks: 1,
    };
    assert_eq!(
        ext.decide(13, 99, &jobs, 3, 3, &code, 1),
        expected(Reason::Reserved, 0, Some(reserved))
    );
    assert_eq!(
        ext.decide(14, 99, &jobs, 3, 3, &code, 1),
        expected(Reason::InvalidObservation, 0, Some(fallback()))
    );
    assert_eq!(
        ext.decide(14, 100, &jobs, 3, 3, &code, 1),
        expected(Reason::Stalled, 0, Some(fallback()))
    );
    assert_eq!(
        ext.decide(14, 101, &jobs, 3, 3, &code, 1),
        expected(Reason::Accepted, 1, Some(p))
    );
    assert_eq!(
        ext.decide(13, 500, &jobs, 3, 3, &code, 1),
        expected(Reason::InvalidObservation, 0, None)
    );
    assert_eq!(
        ext.decide(16, 101, &jobs, 3, 3, &code, 1),
        expected(Reason::Stalled, 0, Some(fallback()))
    );
}

#[test]
fn independent_reservation_blocks_crossing_grants_and_never_lends_an_empty_slot() {
    let mut ext = Extension::new(
        Reservation {
            ticks: 2,
            ..reservation()
        },
        20,
        0,
        30,
    )
    .unwrap();
    let mut jobs = tasks();
    let p = request();
    for base in [20, 25] {
        assert_eq!(
            ext.decide(base + 1, 0, &jobs, 3, 3, &[Op::Propose(p)], 1),
            expected(Reason::Accepted, 1, Some(p))
        );
        assert_eq!(
            ext.decide(base + 2, 0, &jobs, 3, 3, &[Op::Propose(p)], 1),
            expected(Reason::Rejected, 1, Some(fallback()))
        );
        for t in [base + 3, base + 4] {
            let reserved = Proposal {
                task: 2,
                generation: 12,
                cpu: 1,
                ticks: 1,
            };
            assert_eq!(
                ext.decide(t, 0, &jobs, 3, 3, &[Op::Jump(0)], 16),
                expected(Reason::Reserved, 0, Some(reserved))
            );
            jobs[2].grant_limit = 0;
            assert_eq!(
                ext.decide(t, 0, &jobs, 3, 3, &[Op::Propose(p)], 1),
                expected(Reason::Reserved, 0, None)
            );
            jobs[2].grant_limit = 3;
        }
    }
    jobs[0].ready = false;
    jobs[1].ready = false;
    assert_eq!(
        ext.decide(30, 0, &jobs, 3, 3, &[Op::Yield], 1),
        expected(Reason::Yielded, 1, None)
    );
}

#[test]
fn rejects_invalid_domains_and_bounds_arithmetic_near_the_u64_horizon() {
    let r = reservation();
    assert!(Extension::<0>::new(r, 0, 0, 1).is_none());
    assert!(Extension::<33>::new(r, 0, 0, 1).is_none());
    for bad in [
        Reservation { task: 3, ..r },
        Reservation { period: 0, ..r },
        Reservation { offset: 5, ..r },
        Reservation { ticks: 0, ..r },
        Reservation {
            offset: u64::MAX - 1,
            period: u64::MAX,
            ticks: 2,
            ..r
        },
    ] {
        assert!(Extension::<3>::new(bad, 0, 0, 1).is_none());
    }
    assert!(Extension::<3>::new(r, 0, 0, 0).is_none());
    let mut ext = Extension::new(r, u64::MAX - 2, 0, 5).unwrap();
    let jobs = tasks();
    let code = [Op::Propose(request())];
    assert_eq!(
        ext.decide(u64::MAX - 2, 0, &jobs, 3, 3, &code, 1),
        expected(Reason::Accepted, 1, Some(request()))
    );
    assert_eq!(
        ext.decide(u64::MAX - 1, 0, &jobs, 3, 3, &code, 1),
        expected(Reason::Rejected, 1, Some(fallback()))
    );
    assert_eq!(
        ext.decide(u64::MAX, 0, &jobs, 3, 3, &code, 1),
        expected(Reason::Rejected, 1, None)
    );
}

#[test]
fn nine_timer_boundaries_preserve_each_reserved_slot_through_distinct_failure_modes() {
    let reserved = Reservation {
        task: 0,
        period: 3,
        offset: 2,
        ticks: 1,
    };
    let mut ext = Extension::new(reserved, 0, 0, 2).unwrap();
    let task = DelegatedTask {
        generation: 7,
        ready: true,
        allowed_cpus: 1,
        grant_limit: 1,
    };
    let mut jobs = [
        task,
        task,
        DelegatedTask {
            generation: 17,
            ..task
        },
    ];
    let valid = Proposal {
        task: 2,
        generation: 17,
        cpu: 0,
        ticks: 1,
    };
    let branch = [
        Op::IfReady {
            task: 2,
            yes: 1,
            no: 0,
        },
        Op::Propose(valid),
    ];
    let bad_cpu = [Op::Propose(Proposal { cpu: 1, ..valid })];
    let stale = [Op::Propose(Proposal {
        generation: 16,
        ..valid
    })];
    let loop_code = [Op::Jump(0)];
    let cases: [(&[Op], Reason, u8, Option<usize>); 9] = [
        (&branch, Reason::Accepted, 2, Some(2)),
        (&bad_cpu, Reason::Rejected, 1, Some(1)),
        (&loop_code, Reason::Reserved, 0, Some(0)),
        (&stale, Reason::Rejected, 1, Some(1)),
        (&loop_code, Reason::Fuel, 5, Some(1)),
        (&loop_code, Reason::Reserved, 0, Some(0)),
        (&branch, Reason::Stalled, 0, Some(1)),
        (&branch, Reason::Stalled, 0, None),
        (&branch, Reason::Reserved, 0, Some(0)),
    ];
    for (slot, (code, reason, steps, owner)) in cases.into_iter().enumerate() {
        if slot == 7 {
            jobs[1].ready = false;
            jobs[2].ready = false;
        }
        let decision = ext.decide(slot as u64, slot.min(4) as u64, &jobs, 1, 1, code, 5);
        assert_eq!((decision.reason, decision.steps), (reason, steps));
        assert_eq!(
            decision.selected.map(|p| (p.task, p.ticks)),
            owner.map(|id| (id, 1))
        );
    }
}

#[test]
fn normal_mailbox_yield_and_completed_delegates_preserve_the_reserved_window() {
    let reservation = Reservation { task: 0, period: 3, offset: 2, ticks: 1 };
    let mut ext = Extension::new(reservation, 0, 0, 2).unwrap();
    let producer = Proposal { task: 2, generation: 17, cpu: 0, ticks: 1 };
    let program = [Op::IfReady { task: 2, yes: 1, no: 2 }, Op::Propose(producer), Op::Yield];
    let mut jobs = [7, 11, 17].map(|generation| DelegatedTask {
        generation, ready: true, allowed_cpus: 1, grant_limit: 1,
    });
    let mut progress = 0;
    for slot in 0..9 {
        let phase = slot % 3;
        jobs[1].ready = phase == 1;
        jobs[2].ready = phase != 1 && slot < 8;
        progress += u64::from(phase != 2);
        let choice = ext.decide(slot, progress, &jobs, 1, 1, &program, 5);
        let (reason, id, steps) = [(Reason::Accepted, 2, 2), (Reason::Yielded, 1, 2),
                                   (Reason::Reserved, 0, 0)][phase as usize];
        assert_eq!(choice.reason, reason);
        assert_eq!(choice.steps, steps);
        let grant = choice.selected.unwrap();
        assert_eq!((grant.task, grant.generation, grant.cpu, grant.ticks), (id, jobs[id].generation, 0, 1));
    }
}
