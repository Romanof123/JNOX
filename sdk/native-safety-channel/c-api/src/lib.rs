//! Native C ABI for the shared IPC-010 codec.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::panic::PanicInfo;
use jnox_safety_channel::{decode, encode, Error, Permit, FRAME_BYTES};

const ARGUMENT: i32 = -1;

#[repr(C)]
pub struct PermitFields {
    pub generation: u64,
    pub sequence: u64,
    pub issue_ticks: u64,
    pub permit: u64,
}

/// # Safety
/// For a non-null pointer and the requested frame size, `frame` must
/// designate writable storage for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn jnox_permit_encode(fields: PermitFields, frame: *mut u8, length: usize) -> i32 {
    if frame.is_null() {
        return ARGUMENT;
    }
    if length != FRAME_BYTES {
        return Error::Length as i32;
    }
    if fields.permit > 1 {
        return ARGUMENT;
    }
    let bytes = match encode(Permit {
        generation: fields.generation,
        sequence: fields.sequence,
        issue: fields.issue_ticks,
        permit: fields.permit == 1,
    }) {
        Ok(bytes) => bytes,
        Err(error) => return error as i32,
    };
    // The C caller provides writable byte-aligned storage, possibly uninitialized.
    unsafe { frame.cast::<[u8; FRAME_BYTES]>().write(bytes) };
    0
}

/// # Safety
/// For non-null pointers and the requested frame size, `frame` must hold
/// initialized private bytes and `out` must designate a separate aligned,
/// writable PermitFields. The caller retains these objects throughout the call.
#[no_mangle]
pub unsafe extern "C" fn jnox_permit_decode(frame: *const u8, length: usize, out: *mut PermitFields) -> i32 {
    if frame.is_null() || out.is_null() {
        return ARGUMENT;
    }
    if length != FRAME_BYTES {
        return Error::Length as i32;
    }
    // The caller owns this initialized 44-byte snapshot. A byte array needs alignment 1.
    let bytes = unsafe { &*frame.cast::<[u8; FRAME_BYTES]>() };
    let fields = match decode(bytes) {
        Ok(fields) => fields,
        Err(error) => return error as i32,
    };
    // A successful decode initializes the entire separate C output object.
    unsafe {
        out.write(PermitFields {
            generation: fields.generation,
            sequence: fields.sequence,
            issue_ticks: fields.issue,
            permit: u64::from(fields.permit),
        })
    };
    0
}

extern "C" {
    fn __jnox_exit(status: i32) -> !;
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // The ordinary native SDK startup object supplies this process exit routine.
    unsafe { __jnox_exit(111) }
}
