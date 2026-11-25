#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::time::Duration;
use jnox_native::{exit, monotonic_time, sleep_until};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Observation { counter: u64, completed: u64, witness: u64 }

#[no_mangle]
static mut records: [Observation; 8192] = [Observation { counter: 0, completed: 0, witness: 0 }; 8192];
#[no_mangle]
static mut record_count: u32 = 0;
#[no_mangle]
static mut record_overflow: u32 = 0;
#[no_mangle]
static mut private_blocks: u64 = 0;
#[no_mangle]
static mut private_result: [u64; 9] = [0; 9];
#[no_mangle]
static mut first_counter: u64 = 0;
#[no_mangle]
static mut final_counter: u64 = 0;
#[no_mangle]
static mut observed_frequency: u64 = 0;
#[no_mangle]
static mut request_ns: u64 = 0;
#[no_mangle]
static mut observed_ns: u64 = 0;
static mut RAMP: [u8; 256] = [0; 256];

fn counter() -> u64 {
    let value;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

unsafe fn append(t: u64, completed: u64, witness: u64) {
    let i = read_volatile(addr_of!(record_count));
    if i == 8192 {
        write_volatile(addr_of_mut!(record_overflow), 1);
        return;
    }
    write_volatile(addr_of_mut!(records[i as usize]), Observation { counter: t, completed, witness });
    write_volatile(addr_of_mut!(record_count), i + 1);
}

#[no_mangle]
#[inline(never)]
unsafe fn density_block(bins: &mut [u64; 9]) -> bool {
    for i in 0..256 {
        let value = read_volatile(addr_of!(RAMP[i]));
        let bin = &mut bins[value.count_ones() as usize];
        let Some(next) = bin.checked_add(1) else { return false };
        *bin = next;
    }
    true
}

fn sleep_once() -> Option<()> {
    let target = monotonic_time().ok()?.checked_add(Duration::from_millis(160))?;
    unsafe { write_volatile(addr_of_mut!(request_ns), target.as_nanos().try_into().ok()?) };
    sleep_until(target).ok()?;
    let now = monotonic_time().ok()?;
    unsafe { write_volatile(addr_of_mut!(observed_ns), now.as_nanos().try_into().ok()?) };
    (now >= target).then_some(())
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let start = counter();
    let frequency: u64;
    asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags));
    write_volatile(addr_of_mut!(first_counter), start);
    write_volatile(addr_of_mut!(observed_frequency), frequency);
    append(start, 0, 0);
    if frequency == 0 || frequency % 1000 != 0 { return 112; }
    if argc == 2 {
        let arg = *argv.add(1);
        if arg.is_null() { return 113; }
        for (i, &c) in b"sleep-160\0".iter().enumerate() {
            if *arg.add(i) != c { return 113; }
        }
        if sleep_once().is_none() { return 114; }
    } else if argc != 1 { return 113; }

    for i in 0..256 { write_volatile(addr_of_mut!(RAMP[i]), i as u8); }
    let mut bins = [0u64; 9];
    let mut completed = 0u64;
    let mut last = start;
    let threshold = frequency.div_ceil(5000);
    let duration = frequency / 1000 * 900;
    loop {
        if !density_block(&mut bins) { return 115; }
        let Some(next) = completed.checked_add(1) else { return 115 };
        completed = next;
        write_volatile(addr_of_mut!(private_result), bins);
        write_volatile(addr_of_mut!(private_blocks), completed);
        let now = counter();
        if now < last { return 116; }
        if now - last >= threshold {
            append(now, completed, bins[4]);
            last = now;
        }
        if now - start >= duration {
            write_volatile(addr_of_mut!(final_counter), now);
            for (count, per_block) in bins.iter().zip([1,8,28,56,70,56,28,8,1]) {
                if completed.checked_mul(per_block) != Some(*count) { return 117; }
            }
            return 42;
        }
    }
}
