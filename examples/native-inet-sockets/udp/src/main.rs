#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile, time::Duration};
use jnox_native::{exit, local_socket as s, monotonic_time, sleep_until};

#[no_mangle]
static mut inet_udp_result: [i64; 16] = [0; 16];

fn put(i: usize, n: i64) {
    unsafe {
        write_volatile(&raw mut inet_udp_result[i], n);
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

fn run() -> Result<(), i32> {
    s::set_broker(2)?;
    let fd = s::socket4(s::SOCK_DGRAM, false)?;
    let guest = s::Ipv4Endpoint {
        address: [10, 0, 2, 15],
        port: 43003,
    };
    let peer = s::Ipv4Endpoint {
        address: [10, 0, 2, 2],
        port: 43004,
    };
    s::bind4(fd, guest)?;
    for i in 1u8..=4 {
        let n = s::send_to4(fd, &[b'U', i, 10 * i], 0, peer)?;
        if n != 3 {
            return Err(-1000);
        }
        put(i as usize - 1, n as i64);
    }
    let mut data = [0u8; 8];
    for (index, id) in [1u8, 4, 3, 0].iter().copied().enumerate() {
        let (n, from) = s::recv_from4(fd, &mut data, 0)?;
        let from = from.ok_or(-1001)?;
        put(4 + index, i64::from(data[1]));
        put(8, i64::from(u32::from_be_bytes(from.address)));
        put(9, i64::from(from.port));
        if n != 3
            || data[..3] != [b'R', id, 10 * id]
            || from.address != peer.address
            || from.port != peer.port
        {
            return Err(-1002);
        }
    }
    // Observe a bounded post-marker window; the relay log independently records
    // the intentional decision to withhold record 2.
    let end = monotonic_time().map_err(|_| -1003)? + Duration::from_millis(100);
    let mut polls = 0;
    loop {
        match s::recv_from4(fd, &mut data, s::MSG_DONTWAIT) {
            Err(-11) => polls += 1,
            _ => return Err(-1004),
        }
        let now = monotonic_time().map_err(|_| -1003)?;
        if now >= end {
            break;
        }
        sleep_until((now + Duration::from_millis(10)).min(end)).map_err(|_| -1003)?;
    }
    put(10, polls);
    if s::send_to4(fd, b"DONE", 0, peer)? != 4 {
        return Err(-1005);
    }
    s::close(fd)?;
    put(11, 1);
    Ok(())
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    match run() {
        Ok(()) => 76,
        Err(e) => {
            put(15, i64::from(e));
            113
        }
    }
}
