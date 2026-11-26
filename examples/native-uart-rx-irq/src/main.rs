#![no_std]
#![no_main]

use core::{panic::PanicInfo, time::Duration};
use jnox_native::{display::{self, RxMetric}, exit, monotonic_time, sleep_until};

// The first scanline is a handshake visible to QMP, independent of the UART
// under test. A stage is published in one row submission after all its fields.
type Row = [u32; display::WIDTH];
const MARKER: u32 = 0x495200;
const METRICS: [RxMetric; 7] = [
    RxMetric::IrqEntries, RxMetric::Bytes, RxMetric::Drops, RxMetric::ErrorWords,
    RxMetric::Depth, RxMetric::Capacity, RxMetric::IdleIrqEntries,
];

fn now() -> Result<Duration, i64> {
    monotonic_time().map_err(|_| 90)
}

fn sleep() -> Result<(), i64> {
    let deadline = now()? + Duration::from_millis(100);
    sleep_until(deadline).map_err(|_| 91)?;
    if now()? < deadline { return Err(92); }
    Ok(())
}

fn show(stage: u32, row: &mut Row) -> Result<(), i64> {
    row[0] = MARKER | stage;
    display::write_row(0, row)
}

fn key() -> Result<u8, i64> {
    let deadline = now()? + Duration::from_secs(8);
    while now()? < deadline {
        match display::poll_key() {
            Err(-11) => sleep()?,
            other => return other,
        }
    }
    Err(93)
}

fn sample(stage: u32, target: usize) -> Result<(), i64> {
    let started = now()?;
    loop {
        // No data-register or dequeue operation until the IRQ handler itself
        // accounts for the entire transmission. Timer wakes are independent.
        sleep()?;
        if display::rx_stat(RxMetric::Bytes)? >= target { break; }
        if now()? - started > Duration::from_secs(8) { return Err(94); }
    }
    let mut row = [0; display::WIDTH];
    for (i, metric) in METRICS.into_iter().enumerate() {
        row[1 + i] = u32::try_from(display::rx_stat(metric)?).map_err(|_| 95)?;
    }
    row[8] = (now()? - started).as_millis() as u32;
    let depth = row[5] as usize;
    if depth > 16 { return Err(96); }
    for pixel in &mut row[9..9 + depth] {
        *pixel = u32::from(display::poll_key()?);
    }
    if display::poll_key() != Err(-11) { return Err(97); }
    show(stage, &mut row)
}

fn run_owner() -> Result<i32, i64> {
    match display::init() {
        Err(-38) if display::rx_stat(RxMetric::Capacity) == Err(-38) => return Ok(38),
        other => other?,
    }
    if display::poll_key() != Err(-11) { return Err(98); }
    let mut row = [0; display::WIDTH];
    match display::rx_stat(RxMetric::Capacity) {
        Err(-38) => {
            show(4, &mut row)?;
            return Ok(if key()? == b'p' { 73 } else { 99 });
        }
        Ok(16) => {}
        _ => return Err(100),
    }
    if display::rx_stat(RxMetric::Bytes)? != 0 { return Err(101); }
    show(1, &mut row)?;
    sample(2, 8)?;
    sample(3, 32)?;
    Ok(if key()? == b'q' { 70 } else { 102 })
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(argc: i32, _: *const *const u8) -> i32 {
    if argc == 2 && display::poll_key() == Err(-1) {
        return match display::rx_stat(RxMetric::IrqEntries) {
            Err(-1) => 71,
            Err(-38) => 72,
            _ => 103,
        };
    }
    if argc != 1 { return 104; }
    run_owner().unwrap_or_else(|e| e as i32)
}
