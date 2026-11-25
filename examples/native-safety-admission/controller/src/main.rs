#![no_std]
#![no_main]
#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{Case, Phase, Result, Segment, Time, JOBS, SKIP};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, loan, safety_channel as dev};
use jnox_safety_channel::{encode, validate_loan, Permit, Receiver, FRAME_BYTES};

// One row per physical segment, phase A before phase B in each job.
#[no_mangle]
static mut admission_controller_rows: [[u64; 18]; 2 * JOBS] = [[0; 18]; 2 * JOBS];
#[no_mangle]
static mut admission_boundary_inputs: [u64; 11] = [0; 11];
#[no_mangle]
static mut admission_overrun: [u64; 3] = [0; 3];

fn receive(model: &mut Receiver) -> Result<[u64; 4]> {
    let msg = match loan::receive() {
        Ok(m) => m,
        Err(e) => return Ok([e as u64, SKIP, SKIP, 0]),
    };
    let h = msg.handle;
    let slot = (h & 3) as usize;
    if validate_loan(0, 1, slot, msg.length).is_err() || msg.address() != 0x01600000 + 4096 * slot {
        loan::release(h).map_err(|_| 89)?;
        return Err(91);
    }
    let mut bytes = [0; FRAME_BYTES];
    // An authenticated, live read grant is held across this bounded private copy.
    // The supervisor is lower priority, and the sender stays alive until teardown.
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = unsafe { msg.read_page_byte(i) };
    }
    loan::release(h).map_err(|_| 89)?;
    let mut staged = *model;
    let mut clock = None;
    let accepted = staged.accept(&bytes, || {
        clock = dev::info().ok().map(|s| s.ticks);
        clock.unwrap_or(u64::MAX)
    });
    let tick = clock.ok_or(82)?;
    if let Err(e) = accepted {
        *model = staged; // Accept includes an independent old-frame expiry check.
        return Ok([0, e as u64, SKIP, tick]);
    }
    let s = staged.snapshot();
    let commit = dev::commit(s.generation, s.sequence, s.issue.ok_or(91)?, s.permit);
    if commit.is_ok() {
        *model = staged;
    } else {
        model.check(common::info()?.ticks).map_err(|_| 91)?;
    }
    Ok([0, 0, common::rc(commit), tick])
}

fn boundary_inputs(model: Receiver, hz: u64) -> Result<()> {
    if unsafe { admission_boundary_inputs[0] != 0 } {
        return Ok(());
    }
    let s = model.snapshot();
    if s.sequence != 3 || !s.enabled {
        return Ok(());
    }
    let issue = s.issue.ok_or(91)?;
    let expiry = Time { epoch: issue, hz }.at(20_000)?;
    let before = expiry.checked_sub(1).ok_or(83)?;
    let next = Time { epoch: before, hz }.at(5_000)?;
    // These deterministic clocks are inputs to private clones only. One proves
    // equality arbitration; the other has precisely one 5 ms inter-check interval.
    let mut same = model;
    same.begin_activation(before).map_err(|_| 91)?;
    let frame = encode(Permit {
        generation: s.generation,
        sequence: 4,
        issue: expiry,
        permit: true,
    })
    .map_err(|_| 91)?;
    let mut gap = model;
    let early = gap.begin_activation(before).map_err(|_| 91)?;
    let late = gap.begin_activation(next).map_err(|_| 91)?;
    let equal = same.accept(&frame, || expiry);
    unsafe {
        write_volatile(
            &raw mut admission_boundary_inputs,
            [
                1,
                issue,
                before,
                early as u64,
                next,
                late as u64,
                expiry,
                equal.err().map_or(0, |e| e as u64),
                same.snapshot().sequence,
                same.snapshot().enabled as u64,
                gap.snapshot().enabled as u64,
            ],
        );
    }
    Ok(())
}

// A finite negative control: stop at the first observed 1.1 ms overrun or at
// the fixed sampling cap. Only the actual measured interval counts as a witness.
fn overrun(t: Time) -> Result<()> {
    let start = common::info()?.ticks;
    let threshold = Time {
        epoch: start,
        hz: t.hz,
    }
    .at(1_100)?;
    let mut end = start;
    let mut samples = 0;
    for _ in 0..4096 {
        // A batch of ordinary, preemptible EL0 operations per clock read.
        let mut v = core::hint::black_box(end);
        for _ in 0..128 {
            v = core::hint::black_box(v.rotate_left(5).wrapping_add(0x501));
        }
        core::hint::black_box(v);
        end = common::info()?.ticks;
        samples += 1;
        if end >= threshold {
            break;
        }
    }
    unsafe {
        write_volatile(&raw mut admission_overrun, [start, end, samples]);
    }
    Ok(())
}

fn run(case: Case, t: Time) -> Result<()> {
    let mut model = Receiver::new(t.hz).map_err(|_| 91)?;
    for k in 0..JOBS {
        let mut command_used = false;
        for (n, phase) in [Phase::A, Phase::B].into_iter().enumerate() {
            let target = t.at(5_000 * k as u64 + 2_500 * n as u64)?;
            let seg = Segment {
                job: k,
                phase,
                target,
            };
            seg.start()?;
            dev::check().map_err(|_| 92)?; // Both phases make a real expiry-first decision.
            let entry = common::info()?;
            if n == 0 {
                model.begin_activation(entry.ticks)
            } else {
                model.check(entry.ticks)
            }
            .map_err(|_| 91)?;
            let mut command = [SKIP; 3];
            if !command_used && model.snapshot().generation != entry.generation {
                command_used = true;
                model
                    .open(2, entry.generation, entry.ticks)
                    .map_err(|_| 91)?;
                command = [1, 0, SKIP];
            }
            let input = receive(&mut model)?; // One poll per phase, including empty/error attempts.
            let now = common::info()?;
            if !command_used && now.flags & dev::PENDING_REARM != 0 {
                command_used = true;
                let mut staged = model;
                let local = staged.rearm(2, now.generation, now.ticks);
                command = [2, local.err().map_or(0, |e| e as u64), SKIP];
                if local.is_ok() {
                    let applied = dev::process_rearm();
                    command[2] = common::rc(applied);
                    if applied.is_ok() {
                        model = staged;
                    }
                }
            }
            if case == Case::Overrun && k == 4 && n == 0 {
                overrun(t)?;
            }
            if matches!(case, Case::BoundaryWitness | Case::Collision) {
                boundary_inputs(model, t.hz)?;
            }
            dev::complete().map_err(|_| 92)?;
            let end = common::info()?;
            model.check(end.ticks).map_err(|_| 91)?;
            let private = model.snapshot();
            let i = 2 * k + n;
            unsafe {
                write_volatile(
                    &raw mut admission_controller_rows[i],
                    [
                        target,
                        entry.ticks,
                        end.ticks,
                        input[0],
                        input[1],
                        input[2],
                        input[3],
                        command[0],
                        command[1],
                        command[2],
                        end.flags,
                        end.last_sequence,
                        end.last_issue_ticks,
                        private.sequence,
                        private.issue.unwrap_or(0),
                        private.recovery as u64,
                        private.enabled as u64,
                        private.faulted as u64,
                    ],
                );
            }
            common::completed(i + 1);
            seg.finish()?;
        }
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
        let t = Time::join(case)?;
        common::header(case, t);
        let r = run(case, t);
        common::record_result(r);
        common::park()?;
        r
    })()
    .err()
    .unwrap_or(72)
}
