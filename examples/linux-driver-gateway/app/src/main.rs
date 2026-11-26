#![no_std]
#![no_main]

use core::{
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
    time::Duration,
};
use jnox_native::{
    driver_gateway::{Gateway, Outcome, Request},
    exit,
    local_socket::Ipv4Endpoint,
    monotonic_time,
};

// Target evidence ABI v1. Write each phase last, after its measurements.
#[no_mangle]
static mut jnox_gateway_result: [u64; 24] = [0; 24];

fn put(index: usize, value: u64) {
    unsafe { write_volatile(&raw mut jnox_gateway_result[index], value) }
}

fn ns(time: Duration) -> Result<u64, i32> {
    u64::try_from(time.as_nanos()).map_err(|_| -75)
}

fn now_ns() -> Result<u64, i32> {
    ns(monotonic_time().map_err(|_| -5)?)
}

fn run() -> Result<(), i32> {
    const ENABLE: usize = 17; // Software output controlled by the diagnostic client.
    put(0, 1);
    put(ENABLE, 0);
    let mut gateway = Gateway::bind(
        2,
        Ipv4Endpoint {
            address: [10, 0, 2, 15],
            port: 43003,
        },
        Ipv4Endpoint {
            address: [10, 0, 2, 2],
            port: 43004,
        },
    )?;
    let token = now_ns()?;
    let first = Request {
        generation: 1,
        id: 1,
        timeout_ms: 10_000,
        payload: token.to_be_bytes(),
    };
    put(2, token);
    put(1, 1);
    let reply = gateway.exchange(first)?;
    put(3, ns(reply.started)?);
    put(4, ns(reply.finished)?);
    let Outcome::Reply(data) = reply.outcome else {
        return Err(-101);
    };
    put(5, u64::from_be_bytes(data));
    put(ENABLE, 1);
    put(15, 1);
    put(1, 2);

    let outage = Request {
        id: 2,
        timeout_ms: 250,
        ..first
    };
    let missed = gateway.exchange(outage)?;
    if !matches!(missed.outcome, Outcome::TimedOut) {
        return Err(-102);
    }
    put(ENABLE, 0);
    let fallback_ns = now_ns()?;
    put(6, ns(missed.started)?);
    put(7, ns(missed.finished)?);
    put(8, ns(missed.deadline)?);
    put(9, fallback_ns);
    put(16, 1);
    put(18, unsafe {
        read_volatile(&raw const jnox_gateway_result[ENABLE])
    });
    put(1, 3);

    // Deliberately reuse the first request's ID, budget and payload so that the
    // old genuine success is distinguishable from the new one by generation.
    let renewed = gateway.exchange(Request {
        generation: 2,
        ..first
    })?;
    put(10, ns(renewed.started)?);
    put(11, ns(renewed.finished)?);
    put(13, u64::from(renewed.stale_replies));
    put(
        14,
        u64::from(reply.invalid_replies)
            + u64::from(missed.invalid_replies)
            + u64::from(renewed.invalid_replies),
    );
    if let Some(time) = renewed.last_stale {
        put(20, ns(time)?);
    }
    let Outcome::Reply(data) = renewed.outcome else {
        return Err(-103);
    };
    if renewed.stale_replies == 0 {
        return Err(-104);
    }
    put(12, u64::from_be_bytes(data));
    put(ENABLE, 1);
    put(15, 2);
    put(1, 4);
    gateway.close()?;
    put(1, 5);
    Ok(())
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    put(17, 0);
    exit(111)
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    match run() {
        Ok(()) => 81,
        Err(code) => {
            put(17, 0);
            put(19, i64::from(code) as u64);
            112
        }
    }
}
