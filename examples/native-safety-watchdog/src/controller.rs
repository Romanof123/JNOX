use crate::{
    active, inhibited,
    schedule::{info, rc, Phase, Result, Segment, Time, SKIP},
    Case,
};
use core::{arch::asm, ptr::write_volatile, time::Duration};
use jnox_native::{exit, loan, monotonic_time, safety_channel as dev, sleep_until};
use jnox_safety_channel::{validate_loan, Receiver, FRAME_BYTES};

#[no_mangle]
static mut wd_controller_rows: [[u64; 12]; 29] = [[0; 12]; 29];
#[no_mangle]
static mut wd_injection: [u64; 12] = [0; 12];
#[no_mangle]
static mut wd_calibration: [u64; 4] = [0; 4];

// The decoded ELF symbol provides independent boundaries for an interrupted PC.
// All paths through this finite arithmetic loop have zero SVC instructions.
#[no_mangle]
#[inline(never)]
pub extern "C" fn wd_compute_loop(rounds: u64) -> u64 {
    let mut value = 0x578e12u64;
    if rounds != 0 {
        unsafe {
            asm!("2:", "eor {v}, {v}, {v}, ror #7", "add {v}, {v}, #43",
                "subs {n}, {n}, #1", "b.ne 2b",
                v=inout(reg)value, n=inout(reg)rounds=>_,options(nomem,nostack));
        }
    }
    value
}
pub fn calibrate() -> Result<u64> {
    let probe = 65_536;
    core::hint::black_box(wd_compute_loop(probe));
    let mut fastest = u64::MAX;
    let hz = info()?.hz;
    for _ in 0..3 {
        let start = info()?.ticks;
        core::hint::black_box(wd_compute_loop(probe));
        let elapsed = info()?.ticks.checked_sub(start).ok_or(83)?;
        if elapsed > 0 {
            fastest = fastest.min(elapsed);
        }
    }
    if fastest == u64::MAX {
        return Err(83);
    }
    let goal = Time { epoch: 0, hz }.at(12_000)?;
    let rounds = (u128::from(goal) * u128::from(probe))
        .div_ceil(u128::from(fastest))
        .clamp(u128::from(probe), 32_000_000) as u64;
    unsafe {
        write_volatile(&raw mut wd_calibration, [probe, fastest, goal, rounds]);
    }
    Ok(rounds)
}
fn receive(model: &mut Receiver) -> Result<[u64; 3]> {
    let r = match loan::receive() {
        Ok(r) => r,
        Err(e) => return Ok([e as u64, SKIP, SKIP]),
    };
    let slot = (r.handle & 3) as usize;
    if validate_loan(0, 1, slot, r.length).is_err() || r.address() != 0x01600000 + 4096 * slot {
        loan::release(r.handle).map_err(|_| 86)?;
        return Err(86);
    }
    let mut bytes = [0; FRAME_BYTES];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = unsafe { r.read_page_byte(i) };
    }
    loan::release(r.handle).map_err(|_| 86)?;
    let mut staged = *model;
    let mut clock = None;
    let accepted = staged.accept(&bytes, || {
        clock = dev::info().ok().map(|s| s.ticks);
        clock.unwrap_or(u64::MAX)
    });
    clock.ok_or(82)?;
    if let Err(e) = accepted {
        *model = staged;
        return Ok([0, e as u64, SKIP]);
    }
    let s = staged.snapshot();
    let commit = dev::commit(s.generation, s.sequence, s.issue.ok_or(86)?, s.permit);
    if commit.is_ok() {
        *model = staged;
    }
    Ok([0, 0, rc(commit)])
}
fn inject(case: Case, s: &dev::State, last_complete: u64, rounds: u64) -> Result<()> {
    let guarded = active(s, 1) && s.last_sequence >= 2 && last_complete != 0;
    let mut record = [
        1,
        guarded as u64,
        s.ticks,
        s.flags,
        s.generation,
        s.last_sequence,
        s.last_issue_ticks,
        last_complete,
        rounds,
        SKIP,
        0,
        0,
    ];
    unsafe {
        write_volatile(&raw mut wd_injection, record);
    }
    if !guarded {
        return Err(95);
    }
    match case {
        Case::Exit => exit(72),
        Case::Compute => {
            core::hint::black_box(wd_compute_loop(rounds));
        }
        _ => {
            let wake = monotonic_time()
                .map_err(|_| 87)?
                .checked_add(Duration::from_millis(10))
                .ok_or(87)?;
            sleep_until(wake).map_err(|_| 87)?;
        }
    }
    // CHECK opened this activation before the fault. Its next protected
    // operation really attempts to finish it. A late COMPLETE cannot refresh.
    let late = dev::complete();
    let after = info()?;
    record[9] = rc(late);
    record[10] = after.ticks;
    record[11] = after.flags;
    unsafe {
        write_volatile(&raw mut wd_injection, record);
    }
    if late != Err(-110) || !inhibited(&after) {
        return Err(96);
    }
    Ok(())
}
pub fn run(case: Case, t: Time, rounds: u64) -> Result<()> {
    let mut model = Receiver::new(t.hz).map_err(|_| 86)?;
    let mut last_complete = 0;
    let mut command_used = false;
    for i in 0..29 {
        // The fault intentionally suspends real activations. Never manufacture
        // on-time completions by replaying the skipped 22.5-32.5 ms releases.
        if case != Case::Healthy && (9..14).contains(&i) {
            continue;
        }
        let a = i % 2 == 0;
        let seg = Segment {
            job: i / 2,
            phase: if a { Phase::A } else { Phase::B },
            target: t.at(i as u64 * 2500)?,
        };
        seg.start()?;
        let observed = info()?;
        if a {
            command_used = false;
            model.begin_activation(observed.ticks)
        } else {
            model.check(observed.ticks)
        }
        .map_err(|_| 86)?;
        let mut row = [
            seg.target,
            observed.ticks,
            observed.generation,
            observed.flags,
            SKIP,
            SKIP,
            SKIP,
            SKIP,
            SKIP,
            SKIP,
            observed.flags,
            observed.last_sequence,
        ];
        if i >= 14 && case != Case::Healthy && observed.generation == 1 {
            unsafe {
                write_volatile(&raw mut wd_controller_rows[i], row);
            }
            if !inhibited(&observed) {
                return Err(97);
            }
            seg.finish()?;
            continue;
        }
        let entry = dev::check_and_sample().map_err(|_| 87)?;
        if i == 8 && case != Case::Healthy {
            inject(case, &entry, last_complete, rounds)?;
            seg.finish()?;
            continue;
        }
        if model.snapshot().generation != entry.generation {
            if command_used {
                return Err(86);
            }
            command_used = true;
            model
                .open(2, entry.generation, entry.ticks)
                .map_err(|_| 86)?;
        }
        let sample = receive(&mut model)?;
        row[4..7].copy_from_slice(&sample);
        let cmd = info()?;
        if !command_used && cmd.flags & dev::PENDING_REARM != 0 {
            command_used = true;
            let mut staged = model;
            let local = staged.rearm(2, cmd.generation, cmd.ticks);
            row[7] = local.err().map_or(0, |e| e as u64);
            if local.is_ok() {
                let applied = dev::process_rearm();
                row[8] = rc(applied);
                if applied.is_ok() {
                    model = staged;
                }
            }
        }
        let end = dev::complete_and_sample().map_err(|_| 87)?;
        last_complete = end.ticks;
        row[9] = end.ticks;
        row[10] = end.flags;
        row[11] = end.last_sequence;
        unsafe {
            write_volatile(&raw mut wd_controller_rows[i], row);
        }
        seg.finish()?;
    }
    Ok(())
}
