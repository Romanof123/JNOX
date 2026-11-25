#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, ipc, pubsub};

#[no_mangle]
static mut active_ids: [u64; 8] = [0; 8];
#[no_mangle]
static mut active_data: [[u8; 32]; 8] = [[0; 32]; 8];
#[no_mangle]
static mut active_sizes: [u8; 8] = [0; 8];
#[no_mangle]
static mut active_terminal_wait: i64 = 0;
#[no_mangle]
static mut active_observations: [[u64; 4]; 8] = [[0; 4]; 8];

fn command(expected: u64) -> Result<u64, i64> {
    let (token, word) = ipc::receive()?;
    if word == expected { Ok(token) } else { Err(-1) }
}

fn run() -> Result<(), i64> {
    let start = command(100)?;
    let h = pubsub::subscribe(3, 0)?;
    ipc::reply(start, 100)?;
    for (i, len) in [0usize, 1, 7, 32, 11, 3, 19, 6].into_iter().enumerate() {
        let seq = i as u64 + 1;
        let token = command(seq)?;
        let o = pubsub::observe(3, h)?;
        let peek = pubsub::peek(3, h)?;
        let read = pubsub::receive(3, h, false)?;
        let observed = [o.queued as u64, o.high_water as u64, o.lost, o.front_sequence];
        if observed != [1, 1, 0, seq] || peek.sequence != seq || read.sequence != seq ||
            peek.bytes() != read.bytes() || read.bytes().len() != len { return Err(-2); }
        for (j, &b) in read.bytes().iter().enumerate() {
            if b != (43*seq + 19*j as u64 + (j/2) as u64) as u8 { return Err(-3); }
        }
        unsafe {
            write_volatile(&raw mut active_ids[i], read.sequence);
            write_volatile(&raw mut active_sizes[i], len as u8);
            write_volatile(&raw mut active_observations[i], observed);
            for (j, &b) in read.bytes().iter().enumerate() {
                write_volatile(&raw mut active_data[i][j], b);
            }
        }
        ipc::reply(token, seq)?;
    }
    let done = command(202)?;
    ipc::reply(done, 202)?;
    let terminal = pubsub::receive(3, h, true).err().unwrap_or(0);
    unsafe { write_volatile(&raw mut active_terminal_wait, terminal); }
    if terminal != -32 { return Err(-4); }
    pubsub::unsubscribe(3, h)
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main() -> i32 { if run().is_ok() { 72 } else { 101 } }
