#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile, time::Duration};
use jnox_native::{exit, ipc, local_socket as s, monotonic_time, sleep_until};

#[no_mangle]
static mut extra_peer_result: [u64; 16] = [0; 16];

fn record(i: usize, v: u64) {
    unsafe { write_volatile(&raw mut extra_peer_result[i], v); }
}

fn now() -> Result<u64, i32> {
    Ok(monotonic_time().map_err(|_| -1)?.as_nanos() as u64)
}

fn run(mode: u8) -> Result<(), i32> {
    s::set_broker(2)?;
    let (token, deadline) = ipc::receive().map_err(|e| e as i32)?;
    ipc::reply(token, 0xa1).map_err(|e| e as i32)?;
    if mode == b'b' {
        let mut fds = [-1; 3];
        for (i, fd) in fds.iter_mut().enumerate() {
            *fd = s::socket(s::SOCK_STREAM, false)?;
            record(2*i, now()?);
            s::connect(*fd, b"/extra")?;
            record(2*i+1, now()?);
            if s::send(*fd, &[51+i as u8], 0)? != 1 { return Err(-2); }
        }
        if ipc::call(0, 0xa2, u64::MAX).map_err(|e| e as i32)? != 0xa3 { return Err(-6); }
        for fd in fds { s::close(fd)?; }
        Ok(())
    } else {
        let fd = s::socket(s::SOCK_DGRAM, false)?;
        let target = match mode {
            b'e' => deadline + 50000000,
            b'r' => deadline - 100000000,
            _ => return Err(-3),
        };
        sleep_until(Duration::from_nanos(target)).map_err(|_| -4)?;
        record(0, deadline);
        record(1, now()?);
        let n = s::send_to(fd, &[0x6d], 0, b"/extra")?;
        record(2, now()?);
        record(3, n as u64);
        if n != 1 { return Err(-5); }
        // Keep the descriptor and process live until the owner knows the send completed.
        if ipc::call(0, 0xa2, u64::MAX).map_err(|e| e as i32)? != 0xa3 { return Err(-6); }
        s::close(fd)
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 2 || argv.is_null() || (*argv.add(1)).is_null() { return 110; }
    match run(**argv.add(1)) {
        Ok(()) => 82,
        Err(e) => { record(15, i64::from(e) as u64); 112 }
    }
}
