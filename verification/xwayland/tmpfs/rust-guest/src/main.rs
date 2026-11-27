#![no_std]
#![no_main]

use core::panic::PanicInfo;
use jnox_native::tmpfs::{unlink, File, SeekFrom};

const FILE: &[u8] = b"/tmp/rust-sdk";
const SHARED: &[u8] = b"/tmp/rust-shared";

fn check(condition: bool, code: i32) -> Result<(), i32> {
    if condition { Ok(()) } else { Err(code) }
}

fn writer() -> Result<(), i32> {
    let mut file = File::create_new(FILE)?;
    check(matches!(File::create_new(FILE), Err(-17)), 101)?;
    let data = [b'R'; 70];
    check(file.write(&data)? == 64 && file.write(&data[64..])? == 6, 102)?;
    check(file.seek(SeekFrom::End(-2))? == 68, 103)?;
    check(file.write(b"OK")? == 2, 104)?;
    let mut again = File::open_read_write(FILE)?;
    check(again.seek(SeekFrom::Current(0))? == 0, 105)?;
    check(again.seek(SeekFrom::Start(68))? == 68, 106)?;
    let mut buf = [0; 3];
    check(again.read(&mut buf)? == 2 && &buf[..2] == b"OK", 107)?;
    check(again.read(&mut buf)? == 0, 108)?;
    check(again.seek(SeekFrom::Current(-71)) == Err(-22), 109)?;
    check(again.seek(SeekFrom::Start(u64::MAX)) == Err(-75), 110)?;
    unlink(FILE)?;
    check(matches!(File::open(FILE), Err(-2)), 111)?;
    check(file.seek(SeekFrom::Start(68))? == 68 && file.read(&mut buf)? == 2 &&
          &buf[..2] == b"OK", 112)?;
    again.close()?;
    check(again.read(&mut buf) == Err(-9), 113)?;
    drop(file);

    let mut shared = File::create_new(SHARED)?;
    check(shared.write(b"shared")? == 6, 114)?;
    drop(shared);
    // A per-client table has eight entries. Sequential drops must release each one.
    for _ in 0..12 {
        drop(File::open(SHARED)?);
    }
    Ok(())
}

fn reader() -> Result<(), i32> {
    let mut file = File::open(SHARED)?;
    let mut buf = [0; 6];
    check(file.read(&mut buf)? == buf.len() && &buf == b"shared", 115)?;
    check(file.write(b"x") == Err(-9), 116)?;
    check(matches!(File::open_read_write(SHARED), Err(-13)), 117)?;
    check(matches!(File::create_new(FILE), Err(-13)), 118)?;
    check(unlink(SHARED) == Err(-13), 119)
}

fn denied() -> Result<(), i32> {
    check(matches!(File::open(SHARED), Err(-13)), 120)?;
    check(matches!(File::create_new(FILE), Err(-13)), 121)
}

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 2 || argv.is_null() { return 122; }
    let role = unsafe { *argv.add(1) };
    if role.is_null() { return 122; }
    let mut length = 0;
    while length < 7 && unsafe { *role.add(length) } != 0 { length += 1; }
    let outcome = match unsafe { core::slice::from_raw_parts(role, length) } {
        b"writer" => writer(),
        b"reader" => reader(),
        b"denied" => denied(),
        _ => return 122,
    };
    match outcome {
        Ok(()) => 90,
        Err(status) => status,
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    jnox_native::exit(123)
}
