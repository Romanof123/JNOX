#![no_std]
#![no_main]
#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{Case, Phase, Result, Segment, Time, SKIP};
use core::{panic::PanicInfo, ptr::write_volatile};
use dev::Command;
use jnox_native::{exit, safety_channel as dev};

#[no_mangle]
static mut admission_supervisor_rows: [[u64; 12]; 12] = [[0; 12]; 12];
#[no_mangle]
static mut admission_lower_work: [u64; 4] = [0; 4];

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
fn run(case: Case, t: Time) -> Result<()> {
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
        unsafe {
            write_volatile(
                &raw mut admission_supervisor_rows[k],
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
        common::completed(k + 1);
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
