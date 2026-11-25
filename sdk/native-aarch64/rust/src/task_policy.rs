//! Policy-managed tasks on the native AArch64 v1 ABI.
use core::{arch::asm, num::NonZeroU32};

fn call(op: u64, mut arg: i64) -> i64 {
    unsafe { asm!("svc #0x4a01", in("x8") op, inout("x0") arg, options(nostack)) }
    arg
}

/// Finish the current job. Returns on the next release or with a signed ABI error.
pub fn complete() -> Result<(), i64> {
    match call(80, 0) { 0 => Ok(()), error => Err(error) }
}

/// Record a fault and stop this task. Only a signed ABI error can return.
pub fn fault(code: NonZeroU32) -> i64 {
    call(81, i64::from(code.get()))
}

/// Stop this task with an exit status. Only a signed ABI error can return.
pub fn exit(status: i32) -> i64 {
    call(82, i64::from(status))
}
