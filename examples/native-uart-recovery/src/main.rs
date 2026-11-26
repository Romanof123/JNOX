#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr, time::Duration};
use jnox_native::{display::{self, RxMetric}, exit, monotonic_time, sleep_until};

// Each process has a private copy. The observer walks its own TTBR0 and reads
// these two words via QMP. A heartbeat advances only after *both* denials.
#[no_mangle]
#[link_section = ".data.private_report"]
static mut jnox_recovery_probe: [u32; 2] = [0x5042_0000, 0];

fn mark(stage: u32) {
    unsafe { ptr::write_volatile((&raw mut jnox_recovery_probe).cast(), 0x5042_0000 | stage) }
}

fn now() -> Result<Duration, i64> { monotonic_time().map_err(|_| 90) }

fn sleep(ms: u64) -> Result<(), i64> {
    sleep_until(now()? + Duration::from_millis(ms)).map_err(|_| 91)
}

fn show(stage: u32, words: &[u32]) -> Result<(), i64> {
    let mut row = [0; display::WIDTH];
    row[0] = 0x5243_00 | stage;
    row[1..1 + words.len()].copy_from_slice(words);
    display::write_row(0, &row)
}

fn wait_depth(depth: usize) -> Result<(), i64> {
    let until = now()? + Duration::from_secs(8);
    while display::rx_stat(RxMetric::Depth)? != depth {
        if now()? >= until { return Err(92); }
        sleep(20)?;
    }
    Ok(())
}

fn primary() -> Result<i32, i64> {
    display::init()?;
    if display::poll_key() != Err(-11) { return Err(93); }
    mark(1);
    show(1, &[])?;
    wait_depth(5)?;
    let mode = display::poll_key()?;
    if !matches!(mode, b'f' | b'n') { return Err(94); }
    let pending = display::rx_stat(RxMetric::Depth)? as u32;
    let received = display::rx_stat(RxMetric::Bytes)? as u32;
    show(2, &[mode as u32, pending, received])?;
    // Leave all four stale bytes and the release marker in the owner's FIFO.
    wait_depth(5)?;
    mark(2);
    Ok(if mode == b'f' { 86 } else { 0 })
}

fn peer() -> Result<i32, i64> {
    mark(10);
    for tick in 1..=50u32 {
        sleep(80)?;
        if display::poll_key() != Err(-1) || display::rx_stat(RxMetric::Depth) != Err(-1) {
            return Err(95);
        }
        unsafe { ptr::write_volatile((&raw mut jnox_recovery_probe).cast::<u32>().add(1), tick) };
    }
    mark(11);
    Ok(71)
}

fn standby() -> Result<i32, i64> {
    if display::poll_key() != Err(-19) { return Err(96); }
    mark(20);
    // Leave a visible interval for the observer to sample both interrupt masks.
    sleep(650)?;
    display::init()?;
    for metric in [RxMetric::IrqEntries, RxMetric::Depth, RxMetric::Bytes, RxMetric::Drops,
                   RxMetric::ErrorWords, RxMetric::IdleIrqEntries] {
        if display::rx_stat(metric)? != 0 { return Err(97); }
    }
    if display::rx_stat(RxMetric::Capacity)? != 16 || display::poll_key() != Err(-11) {
        return Err(98);
    }
    mark(21);
    show(3, &[0, 16])?;
    wait_depth(8)?;
    let mut words = [0; 9];
    words[0] = display::rx_stat(RxMetric::Bytes)? as u32;
    for word in &mut words[1..] { *word = display::poll_key()? as u32; }
    if display::poll_key() != Err(-11) { return Err(99); }
    show(4, &words)?;
    wait_depth(1)?;
    if display::poll_key()? != b'q' { return Err(100); }
    mark(22);
    // No further standby is available after this second reported failure.
    Ok(86)
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let result = if argc == 1 {
        primary()
    } else if argc == 2 && !argv.is_null() {
        // The native startup contract supplies NUL-terminated packaged argv.
        let role = *argv.add(1);
        if role.is_null() { return 101; }
        match *role { b'p' => peer(), b's' => standby(), _ => Err(101) }
    } else { Err(101) };
    result.unwrap_or_else(|err| err as i32)
}
