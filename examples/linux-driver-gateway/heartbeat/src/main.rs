#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile, time::Duration};
use jnox_native::{exit, local_socket as s, monotonic_time, sleep_until};

const SAMPLES: usize = 240;
const PERIOD: Duration = Duration::from_millis(50);

// Independent observer ABI: version, sample count, signed errno, period (ns),
// followed by actual monotonic timestamps. Publish count after each sample.
#[no_mangle]
static mut jnox_gateway_heartbeat: [u64; 4 + SAMPLES] = [0; 4 + SAMPLES];

fn put(index: usize, value: u64) {
    unsafe { write_volatile(&raw mut jnox_gateway_heartbeat[index], value) }
}

fn run() -> Result<(), i32> {
    put(0, 1);
    put(3, PERIOD.as_nanos() as u64);
    s::set_broker(2)?;
    match s::socket4(s::SOCK_DGRAM, true) {
        Err(-13) => put(2, (-13_i64) as u64),
        Err(error) => return Err(error),
        Ok(fd) => {
            s::close(fd)?;
            return Err(-105);
        }
    }
    let start = monotonic_time().map_err(|_| -5)?;
    for index in 0..SAMPLES {
        if index != 0 {
            sleep_until(start + PERIOD * index as u32).map_err(|_| -5)?;
        }
        let actual = monotonic_time().map_err(|_| -5)?;
        put(
            4 + index,
            u64::try_from(actual.as_nanos()).map_err(|_| -75)?,
        );
        put(1, (index + 1) as u64);
    }
    Ok(())
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    match run() {
        Ok(()) => 82,
        Err(_) => 113,
    }
}
