use crate::{
    schedule::{info, rc, Phase, Result, Segment, Time, SKIP},
    Case,
};
use core::ptr::write_volatile;
use jnox_native::loan;
use jnox_safety_channel::{encode, Permit, FRAME_BYTES};

#[no_mangle]
static mut wd_sender_rows: [[u64; 10]; 7] = [[0; 10]; 7];
#[no_mangle]
static mut wd_sender_bytes: [[u8; FRAME_BYTES]; 7] = [[0; FRAME_BYTES]; 7];
pub fn run(case: Case, t: Time) -> Result<()> {
    for k in 0..7 {
        let seg = Segment {
            job: k,
            phase: Phase::Sender,
            target: t.at(k as u64 * 10_000)?,
        };
        seg.start()?;
        let s = info()?;
        if case == Case::Exit && k >= 2 {
            unsafe {
                write_volatile(
                    &raw mut wd_sender_rows[k],
                    [
                        seg.target,
                        s.ticks,
                        s.generation,
                        0,
                        0,
                        0,
                        0,
                        SKIP,
                        SKIP,
                        SKIP,
                    ],
                );
            }
            seg.finish()?;
            continue;
        }
        let sequence = if case.recovery() && s.generation == 2 {
            (k.checked_sub(4).ok_or(86)? + 1) as u64
        } else {
            (k + 1) as u64
        };
        // The exact calendar pairs are explicit test inputs. In recover-live,
        // both generation-2 issue stamps are real SVC61 samples.
        let planned = (s.generation == 1 && k < 2)
            || (case == Case::Recover && s.generation == 2 && sequence <= 2);
        let issue = if planned { seg.target } else { s.ticks };
        let data = encode(Permit {
            generation: s.generation,
            sequence,
            issue,
            permit: true,
        })
        .map_err(|_| 86)?;
        let mut row = [
            seg.target,
            s.ticks,
            s.generation,
            sequence,
            issue,
            planned as u64,
            0,
            SKIP,
            SKIP,
            SKIP,
        ];
        match loan::acquire() {
            Ok(w) => {
                row[6] = w.handle;
                row[7] = 0;
                // No writer mapping is retained across jobs. The exit case sends
                // only the two samples the controller already consumed before exit.
                for (i, &b) in data.iter().enumerate() {
                    unsafe {
                        w.write_byte(i, b);
                    }
                }
                let published = loan::publish(w.handle, FRAME_BYTES);
                row[8] = rc(published);
                if published == Err(-11) {
                    row[9] = rc(loan::release(w.handle));
                }
            }
            Err(e) => row[7] = e as u64,
        }
        unsafe {
            write_volatile(&raw mut wd_sender_bytes[k], data);
            write_volatile(&raw mut wd_sender_rows[k], row);
        }
        seg.finish()?;
    }
    Ok(())
}
