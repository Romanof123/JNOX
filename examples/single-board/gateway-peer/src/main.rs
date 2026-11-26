#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile, time::Duration};
use jnox_native::{driver_gateway::bao, exit, monotonic_time, sleep_until};

#[no_mangle]
static mut jnox_gateway_heartbeat: [u64; 136] = [0; 136];

fn put(index: usize, value: u64) {
    unsafe { write_volatile(&raw mut jnox_gateway_heartbeat[index], value) }
}

fn ns(time: Duration) -> Result<u64, i32> {
    u64::try_from(time.as_nanos()).map_err(|_| -75)
}

fn run() -> Result<(), i32> {
    put(0, 1);
    put(1, 1);
    let mut record = [0; 24];
    let sent = bao::send(&record).err().unwrap_or(0);
    let received = bao::receive(&mut record).err().unwrap_or(0);
    put(2, i64::from(sent) as u64);
    put(3, i64::from(received) as u64);
    if (sent, received) != (-13, -13) {
        return Err(-106);
    }
    let mut deadline = monotonic_time().map_err(|_| -5)?;
    put(5, ns(deadline)?);
    put(1, 2);
    let mut count = 0u64;
    loop {
        deadline = deadline.checked_add(Duration::from_millis(20)).ok_or(-75)?;
        sleep_until(deadline).map_err(|_| -5)?;
        let now = monotonic_time().map_err(|_| -5)?;
        let stamp = ns(now)?;
        put(6 + (count % 128) as usize, stamp);
        put(134, stamp);
        count = count.checked_add(1).ok_or(-75)?;
        // External readers can sample the count around a ring read.
        unsafe { core::arch::asm!("dmb ishst", options(nostack, preserves_flags)) };
        put(4, count);
        // Missed periods do not create back-to-back synthetic heartbeat entries.
        if deadline.saturating_add(Duration::from_millis(20)) <= now {
            deadline = now;
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    put(135, (-5i64) as u64);
    exit(111)
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    if let Err(error) = run() {
        put(135, i64::from(error) as u64);
    }
    112
}
