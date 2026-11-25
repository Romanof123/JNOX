use crate::{
    active, inhibited,
    schedule::{info, rc, Phase, Result, Segment, Time, SKIP},
    Case,
};
use core::ptr::write_volatile;
use dev::Command;
use jnox_native::safety_channel as dev;

const TIMES: [u64; 12] = [
    15_000, 17_500, 27_500, 32_500, 35_000, 37_500, 42_500, 52_500, 55_000, 57_500, 67_500, 70_000,
];
#[no_mangle]
static mut wd_supervisor_rows: [[u64; 13]; TIMES.len()] = [[0; 13]; TIMES.len()];
#[no_mangle]
static mut wd_recovery_pair: [u64; 5] = [0; 5];

pub fn run(case: Case, t: Time) -> Result<()> {
    let mut first_active = false;
    let mut observed_fault = false;
    let mut missed = false;
    let mut pair = [None, None];
    let mut eligible = false;
    for (k, us) in TIMES.into_iter().enumerate() {
        let seg = Segment {
            job: k,
            phase: Phase::Supervisor,
            target: t.at(us)?,
        };
        seg.start()?;
        let pre = info()?;
        let mut command = None;
        let mut expected = SKIP;
        match us {
            15_000 => {
                command = Some((Command::Rearm, 1));
                expected = 0;
            }
            17_500 => {
                first_active = active(&pre, 1) && pre.last_sequence == 2;
                missed |= !first_active;
            }
            27_500 if case != Case::Healthy => {
                observed_fault = inhibited(&pre);
                missed |= !first_active || !observed_fault;
                command = Some((Command::Rearm, 1));
                expected = if case == Case::Exit {
                    (-32i64) as u64
                } else {
                    (-11i64) as u64
                };
            }
            32_500 if case != Case::Healthy => missed |= !inhibited(&pre),
            35_000 if case.recovery() && first_active && observed_fault && inhibited(&pre) => {
                command = Some((Command::Open, 2));
                expected = 0;
            }
            35_000 if case == Case::Exit => {
                command = Some((Command::Open, 2));
                expected = (-32i64) as u64;
            }
            37_500 | 42_500 | 52_500 if case.recovery() => {
                missed |=
                    pre.generation != 2 || pre.flags & (dev::ENABLED | dev::WATCHDOG_FAULT) != 0;
                if us != 37_500 {
                    let slot = usize::from(us == 52_500);
                    if pre.last_sequence == 1 + slot as u64 {
                        pair[slot] = Some(pre.last_issue_ticks);
                    } else {
                        missed = true;
                    }
                }
            }
            55_000 if case.recovery() => {
                let exact = match pair {
                    [Some(a), Some(b)] => b
                        .checked_sub(a)
                        .is_some_and(|delta| u128::from(delta) * 1000 == u128::from(t.hz) * 10),
                    _ => false,
                };
                let fresh = pair[0]
                    .and_then(|x| pre.ticks.checked_sub(x))
                    .is_some_and(|age| u128::from(age) * 1000 < u128::from(t.hz) * 20);
                eligible = exact && fresh;
                missed |= pre.generation != 2
                    || pre.flags & dev::ENABLED != 0
                    || (case == Case::Recover && !eligible);
                command = Some((Command::Rearm, 2));
                expected = if eligible { 0 } else { (-11i64) as u64 };
                unsafe {
                    write_volatile(
                        &raw mut wd_recovery_pair,
                        [
                            pair[0].unwrap_or(0),
                            pair[1].unwrap_or(0),
                            pre.ticks,
                            exact as u64,
                            fresh as u64,
                        ],
                    );
                }
            }
            57_500 | 67_500 | 70_000 if case.recovery() => {
                missed |= pre.generation != 2
                    || pre.flags & dev::WATCHDOG_FAULT != 0
                    || (pre.flags & dev::ENABLED != 0) != eligible
                    || pre.flags & dev::WATCHDOG_PRIMED == 0;
            }
            _ => {}
        }
        if us >= 17_500 && case == Case::Healthy {
            missed |= !active(&pre, 1);
        }
        if us >= 27_500 && !case.recovery() && case != Case::Healthy {
            missed |= !inhibited(&pre);
        }
        let status = command.map_or(SKIP, |(cmd, gen)| rc(dev::supervise(cmd, gen)));
        let post = info()?;
        if let Some((Command::Open, 2)) = command {
            if case.recovery() {
                missed |= post.generation != 2 || post.flags != dev::GATE_OPEN;
            }
        }
        missed |= status != expected;
        unsafe {
            write_volatile(
                &raw mut wd_supervisor_rows[k],
                [
                    seg.target,
                    pre.ticks,
                    post.ticks,
                    pre.generation,
                    pre.flags,
                    pre.last_sequence,
                    pre.last_issue_ticks,
                    command.map_or(0, |(cmd, _)| cmd as u64),
                    command.map_or(0, |(_, g)| g),
                    status,
                    post.generation,
                    post.flags,
                    missed as u64,
                ],
            );
        }
        seg.finish()?;
    }
    if missed {
        Err(98)
    } else {
        Ok(())
    }
}
