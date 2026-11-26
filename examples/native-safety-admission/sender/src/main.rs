#![no_std]
#![no_main]
#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{Case, Phase, Result, Segment, Time, SKIP};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, loan};
use jnox_safety_channel::{encode, Permit, FRAME_BYTES};

const N: usize = common::JOBS / 2;
#[no_mangle]
static mut admission_sender_rows: [[u64; 10]; N * 4] = [[0; 10]; N * 4];
#[no_mangle]
static mut admission_sender_count: usize = 0;
#[no_mangle]
static mut admission_sender_bytes: [[u8; FRAME_BYTES]; N * 4] = [[0; FRAME_BYTES]; N * 4];

#[derive(Clone, Copy)]
struct Frame {
    sequence: u64,
    issue: u64,
    data: [u8; FRAME_BYTES],
}
struct Held {
    frame: Frame,
    handle: u64,
}

fn record(
    job: usize,
    target: u64,
    creation: u64,
    f: Frame,
    handle: u64,
    acquire: u64,
    publish: u64,
    retry: bool,
) -> Result<()> {
    let end = common::info()?.ticks;
    unsafe {
        let i = admission_sender_count;
        if i >= N * 4 {
            return Err(88);
        }
        write_volatile(&raw mut admission_sender_bytes[i], f.data);
        write_volatile(
            &raw mut admission_sender_rows[i],
            [
                job as u64,
                target,
                creation,
                end,
                f.sequence,
                f.issue,
                handle,
                acquire,
                publish,
                retry as u64,
            ],
        );
        write_volatile(&raw mut admission_sender_count, i + 1);
    }
    Ok(())
}
fn transmit(
    job: usize,
    target: u64,
    creation: u64,
    f: Frame,
    previous: Option<u64>,
) -> Result<Option<Held>> {
    let handle = if let Some(h) = previous {
        h
    } else {
        let writer = match loan::acquire() {
            Ok(w) => w,
            Err(e) => {
                record(job, target, creation, f, 0, e as u64, SKIP, false)?;
                return Ok(None);
            }
        };
        // Both loan endpoints stay alive. The lower-ranked supervisor cannot
        // close the gate while the sender is filling this mapping.
        for (i, &b) in f.data.iter().enumerate() {
            unsafe {
                writer.write_byte(i, b);
            }
        }
        writer.handle
    };
    let published = loan::publish(handle, FRAME_BYTES);
    record(
        job,
        target,
        creation,
        f,
        handle,
        if previous.is_some() { SKIP } else { 0 },
        common::rc(published),
        previous.is_some(),
    )?;
    if published == Err(-11) {
        return Ok(Some(Held { handle, frame: f }));
    }
    if published.is_err() {
        return Err(89);
    }
    Ok(None)
}
fn run(case: Case, t: Time) -> Result<()> {
    let mut held: Option<Held> = None;
    for k in 0..N {
        let target = t.at(k as u64 * 10_000)?;
        let seg = Segment {
            job: k,
            phase: Phase::Sender,
            target,
        };
        seg.start()?;
        if let Some(h) = held.take() {
            held = transmit(k, target, 0, h.frame, Some(h.handle))?;
        }
        let copies = match case {
            Case::Saturation if k >= 3 => 3,
            Case::Delayed if k < 3 => 0,
            Case::BoundaryWitness if k >= 3 => 0,
            Case::Collision if k == 3 => 0,
            _ => 1,
        };
        for n in 0..copies {
            if held.is_some() {
                break;
            }
            let creation = common::info()?.ticks;
            let seq = match case {
                Case::Delayed if k >= 3 => {
                    if k == 3 {
                        1
                    } else {
                        (k - 3) as u64
                    }
                }
                Case::Collision if k >= 4 => k as u64,
                Case::Invalid | Case::Saturation if k >= 3 => 4,
                _ => (k + 1) as u64,
            };
            // Explicit input calendars give an exactly spaced recovery pair.
            // Live issuance never substitutes a scheduled target for a timestamp.
            let issue = match case {
                Case::Live => creation,
                Case::Delayed if k == 3 => t.epoch,
                Case::Delayed if matches!(k, 4 | 5) => target,
                Case::BoundaryWitness if k == 2 => target.checked_add(1).ok_or(83)?,
                Case::Collision | Case::BoundaryWitness => target,
                _ if k < 2 => target,
                _ => creation,
            };
            let permit = k != 7
                || matches!(
                    case,
                    Case::Invalid
                        | Case::Saturation
                        | Case::Collision
                        | Case::BoundaryWitness
                        | Case::Silence
                        | Case::GateInvalid
                        | Case::GateSaturation
                );
            let mut data = encode(Permit {
                generation: 1,
                sequence: seq,
                issue,
                permit,
            })
            .map_err(|_| 90)?;
            if k >= 3 && matches!(case, Case::Invalid | Case::Saturation) {
                data[40 + n % 4] ^= 1;
            }
            held = transmit(
                k,
                target,
                creation,
                Frame {
                    sequence: seq,
                    issue,
                    data,
                },
                None,
            )?;
        }
        if k + 1 == N {
            if let Some(h) = held.take() {
                loan::release(h.handle).map_err(|_| 89)?;
            }
        }
        common::completed(k + 1);
        seg.finish()?;
    }
    if case.closed_gate() {
        // Check the ordinary upstream path after all eight jobs are recorded.
        if unsafe { core::ptr::read_volatile(&raw const admission_sender_count) } != N {
            return Err(94);
        }
        for k in 0..N {
            let row = unsafe { core::ptr::read_volatile(&raw const admission_sender_rows[k]) };
            let expected = if k < 3 {
                (0, 0)
            } else {
                ((-32_i64) as u64, SKIP)
            };
            if row[0] != k as u64
                || row[4] != (k + 1) as u64
                || (row[7], row[8]) != expected
                || row[9] != 0
            {
                return Err(94);
            }
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
    .unwrap_or(71)
}
