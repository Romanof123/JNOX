//! CPU-only virtual permit bench. Clock readings are raw monotonic ticks.
use core::arch::asm;

#[repr(u64)]
#[derive(Clone, Copy)]
pub enum Command { Open = 1, Rearm = 2, Close = 3, Mark = 4 }

pub const GATE_OPEN: u64 = 1;
pub const ENABLED: u64 = 2;
pub const FAULT: u64 = 4;
pub const PENDING_REARM: u64 = 8;

pub struct State {
    pub ticks: u64,
    pub hz: u64,
    pub generation: u64,
    pub flags: u64,
    pub last_sequence: u64,
    pub last_issue_ticks: u64,
}

fn operation(op: u64, action: u64, generation: u64, sequence: u64,
             issue_ticks: u64, permit: u64) -> Result<(), i64> {
    let status: i64;
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") action => status,
             inout("x1") generation => _, inout("x2") sequence => _,
             inout("x3") issue_ticks => _, inout("x4") permit => _, options(nostack));
    }
    if status == 0 { Ok(()) } else { Err(status) }
}

pub fn supervise(command: Command, generation: u64) -> Result<(), i64> {
    operation(59, command as u64, generation, 0, 0, 0)
}

pub fn check() -> Result<(), i64> { operation(60, 1, 0, 0, 0, 0) }

/// Submit fields from a completely validated private snapshot.
pub fn commit(generation: u64, sequence: u64, issue_ticks: u64, permit: bool) -> Result<(), i64> {
    operation(60, 2, generation, sequence, issue_ticks, u64::from(permit))
}

pub fn complete() -> Result<(), i64> { operation(60, 3, 0, 0, 0, 0) }

pub fn process_rearm() -> Result<(), i64> { operation(60, 4, 0, 0, 0, 0) }

pub fn info() -> Result<State, i64> {
    let (status, ticks, hz, generation, flags, last_sequence, last_issue_ticks);
    unsafe {
        asm!("svc #0x4a01", in("x8") 61u64, inout("x0") 0i64 => status,
             inout("x1") 0u64 => ticks, inout("x2") 0u64 => hz,
             inout("x3") 0u64 => generation, inout("x4") 0u64 => flags,
             inout("x5") 0u64 => last_sequence, inout("x6") 0u64 => last_issue_ticks,
             options(nostack));
    }
    if status != 0 { return Err(status); }
    Ok(State { ticks, hz, generation, flags, last_sequence, last_issue_ticks })
}
