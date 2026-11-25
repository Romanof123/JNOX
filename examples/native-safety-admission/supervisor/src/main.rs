#![no_std]
#![no_main]
#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{Case, Phase, Result, Segment, Time, SKIP};
use core::{panic::PanicInfo, ptr::write_volatile};
use dev::Command;
use jnox_native::{exit, safety_channel as dev};

const GATE_CALENDAR_US: [u64; 13] = [
    15_000, 17_500, 22_500, 25_000, 30_000, 35_000, 40_000, 45_000, 50_000, 55_000, 57_500, 77_500,
    80_000,
];
#[no_mangle]
static mut admission_supervisor_rows: [[u64; 12]; GATE_CALENDAR_US.len()] =
    [[0; 12]; GATE_CALENDAR_US.len()];
#[no_mangle]
static mut admission_lower_work: [u64; 4] = [0; 4];
#[no_mangle]
static mut admission_gate_attempts: [[u64; 5]; 7 * 3] = [[0; 5]; 7 * 3];
#[no_mangle]
static mut admission_gate_attempt_count: usize = 0;

fn bounded_load() -> Result<()> {
    let first = common::info()?.ticks;
    let mut sum = first;
    // Fixed demand and at most 32 finite kernel sections. Timer preemption
    // can interrupt the user computation. A realized overlap requires a trace.
    for _ in 0..32 {
        let sample = common::info()?;
        sum ^= sample.ticks;
        for _ in 0..128 {
            sum = core::hint::black_box(sum.rotate_left(7).wrapping_add(0x912d));
        }
    }
    let last = common::info()?.ticks;
    unsafe {
        write_volatile(&raw mut admission_lower_work, [first, last, 32, sum]);
    }
    Ok(())
}
fn record_job(
    seg: &Segment,
    first: &dev::State,
    last: &dev::State,
    cmd: Option<Command>,
    code: u64,
) {
    unsafe {
        write_volatile(
            &raw mut admission_supervisor_rows[seg.job],
            [
                seg.target,
                first.ticks,
                last.ticks,
                cmd.map_or(0, |c| c as u64),
                code,
                first.flags,
                last.flags,
                first.last_sequence,
                last.last_sequence,
                first.last_issue_ticks,
                last.last_issue_ticks,
                last.generation,
            ],
        );
    }
    common::completed(seg.job + 1);
}

fn run_closed_gate(case: Case, t: Time) -> Result<()> {
    let mut attempts = 0;
    let mut saw_full = false;
    let mut before_expiry = false;
    let mut after_expiry = false;
    let mut issue = 0;
    let mut expiry = 0;
    let mut statuses_ok = true;
    let burst = if case == Case::GateSaturation { 3 } else { 1 };
    for (k, us) in GATE_CALENDAR_US.into_iter().enumerate() {
        let seg = Segment {
            job: k,
            phase: Phase::Supervisor,
            target: t.at(us)?,
        };
        seg.start()?;
        let first = common::info()?;
        let cmd = match us {
            15_000 => Some(Command::Rearm),
            22_500 => Some(Command::Close),
            _ => None,
        };
        let code = cmd.map_or(SKIP, |c| common::rc(dev::supervise(c, 1)));
        if (25_000..=55_000).contains(&us) {
            for n in 0..burst {
                let start = common::info()?.ticks;
                let result = dev::inject_invalid(1);
                let end = common::info()?.ticks;
                unsafe {
                    write_volatile(
                        &raw mut admission_gate_attempts[attempts],
                        [k as u64, n as u64, start, end, common::rc(result)],
                    );
                    write_volatile(&raw mut admission_gate_attempt_count, attempts + 1);
                }
                attempts += 1;
                match result {
                    Ok(()) => {
                        before_expiry |= end < expiry;
                        after_expiry |= start >= expiry;
                    }
                    Err(-11) if case == Case::GateSaturation => saw_full = true,
                    _ => statuses_ok = false,
                }
            }
        }
        let last = common::info()?;
        record_job(&seg, &first, &last, cmd, code);
        seg.finish()?;
        if first.generation != 1 || last.generation != 1 || (cmd.is_some() && code != 0) {
            return Err(95);
        }
        if us == 22_500 {
            let required = dev::GATE_OPEN | dev::ENABLED;
            if first.flags & required != required
                || first.flags & dev::FAULT != 0
                || first.last_sequence != 3
                || first.last_issue_ticks == 0
            {
                return Err(95);
            }
            issue = first.last_issue_ticks;
            expiry = Time {
                epoch: issue,
                hz: t.hz,
            }
            .at(20_000)?;
        }
        if us >= 22_500
            && (last.flags & dev::GATE_OPEN != 0
                || last.last_sequence != 3
                || last.last_issue_ticks != issue)
        {
            return Err(95);
        }
        if us >= 55_000 && (last.flags & (dev::ENABLED | dev::FAULT)) != dev::FAULT {
            return Err(95);
        }
    }
    if !statuses_ok
        || !before_expiry
        || !after_expiry
        || (case == Case::GateSaturation && !saw_full)
    {
        return Err(96);
    }
    Ok(())
}

fn run(case: Case, t: Time) -> Result<()> {
    if case.closed_gate() {
        return run_closed_gate(case, t);
    }
    let mut k = 0;
    for us in [
        15_000, 17_500, 19_900, 22_500, 35_000, 55_000, 57_500, 77_500, 80_000,
    ] {
        if us == 19_900 && case != Case::LowerWork {
            continue;
        }
        let seg = Segment {
            job: k,
            phase: Phase::Supervisor,
            target: t.at(us)?,
        };
        seg.start()?;
        let first = common::info()?;
        let cmd = match (us, case) {
            (55_000, Case::Delayed) => Some(Command::Rearm),
            (15_000, Case::Delayed) => None,
            (15_000, _) => Some(Command::Rearm),
            (22_500, Case::Silence) => Some(Command::Close),
            (
                22_500,
                Case::Invalid | Case::Saturation | Case::Collision | Case::BoundaryWitness,
            ) => Some(Command::Mark),
            _ => None,
        };
        let code = cmd.map_or(SKIP, |c| common::rc(dev::supervise(c, 1)));
        if us == 19_900 {
            bounded_load()?;
        }
        let last = common::info()?;
        record_job(&seg, &first, &last, cmd, code);
        seg.finish()?;
        k += 1;
    }
    Ok(())
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}
#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    (|| {
        let case = Case::parse(argc, argv)?;
        let t = Time::origin(case)?;
        common::header(case, t);
        let r = run(case, t);
        common::record_result(r);
        common::stop()?;
        r
    })()
    .err()
    .unwrap_or(73)
}
