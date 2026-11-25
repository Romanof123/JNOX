#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, local_socket as s};

#[no_mangle]
static mut socket_client_result: [i64; 12] = [0; 12];

fn record(field: usize, value: i64) {
    unsafe {
        write_volatile(&raw mut socket_client_result[field], value);
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

fn expect(condition: bool) -> Result<(), i32> {
    if condition {
        Ok(())
    } else {
        Err(-1000)
    }
}

fn pressure(fd: i32) -> Result<(), i32> {
    let refusal = s::send(fd, &[250], s::MSG_DONTWAIT).err().unwrap_or(0);
    record(5, i64::from(refusal));
    expect(refusal == -11)
}

fn exchange(kind: i32) -> Result<(), i32> {
    s::set_broker(2)?;
    record(0, i64::from(kind));
    let fd = s::socket(kind, false)?;
    let gate = s::socket(s::SOCK_DGRAM, false)?;
    if kind == s::SOCK_DGRAM {
        s::bind(fd, b"/origin")?;
    }
    s::connect(fd, b"/telemetry")?;
    let expected = match kind {
        s::SOCK_STREAM => {
            let n = s::send(fd, &[1, 2, 3, 4, 5, 6, 7], 0)?;
            record(1, n as i64);
            expect(n == 7)?;
            let next: [u8; 64] = core::array::from_fn(|i| (8 + i) as u8);
            let n = s::send(fd, &next, 0)?;
            record(2, n as i64);
            expect(n == 57)?;
            pressure(fd)?;
            s::shutdown(fd, s::SHUT_WR)?;
            32
        }
        s::SOCK_DGRAM => {
            let first = s::send(fd, &[10, 11, 12, 13, 14], 0)?;
            let second = s::send(fd, &[20, 21], 0)?;
            record(1, first as i64);
            record(2, second as i64);
            expect((first, second) == (5, 2))?;
            74
        }
        s::SOCK_SEQPACKET => {
            for i in 1u8..=4 {
                let n = s::send(fd, &[i, 10 + i, 20 + i], 0)?;
                record(i as usize, n as i64);
                expect(n == 3)?;
            }
            pressure(fd)?;
            s::shutdown(fd, s::SHUT_WR)?;
            150
        }
        _ => return Err(-22),
    };
    expect(s::send_to(gate, &[kind as u8], 0, b"/gate")? == 1)?;
    let mut ack = [0; 1];
    if kind == s::SOCK_DGRAM {
        let (n, source) = s::recv_from(fd, &mut ack, 0)?;
        record(7, source.as_bytes().len() as i64);
        expect(n == 1 && source.as_bytes() == b"/telemetry")?;
    } else {
        expect(s::recv(fd, &mut ack, 0)? == 1)?;
        expect(s::recv(fd, &mut [0; 1], 0)? == 0)?;
    }
    record(6, i64::from(ack[0]));
    expect(ack[0] == expected)?;
    s::close(fd)?;
    s::close(gate)?;
    record(8, 1);
    Ok(())
}

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 2 || argv.is_null() {
        return 113;
    }
    let arg = *argv.add(1);
    if arg.is_null() {
        return 113;
    }
    let mut len = 0;
    while len < 7 && *arg.add(len) != 0 {
        len += 1;
    }
    let kind = match core::slice::from_raw_parts(arg, len) {
        b"stream" => s::SOCK_STREAM,
        b"dgram" => s::SOCK_DGRAM,
        b"seq" => s::SOCK_SEQPACKET,
        _ => return 113,
    };
    match exchange(kind) {
        Ok(()) => 74,
        Err(e) => {
            record(11, i64::from(e));
            114
        }
    }
}
