//! Owner-only ramfb scanout and nonblocking PL011 byte input on native virt.
use core::arch::asm;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;

fn call(op: u64, arg: usize, pointer: usize) -> Result<usize, i64> {
    let raw: i64;
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") arg => raw,
             in("x1") pointer, options(nostack));
    }
    if raw < 0 { Err(raw) } else { Ok(raw as usize) }
}

/// Configure a 320×240 XRGB8888 scanout. An absent device returns -ENODEV.
pub fn init() -> Result<(), i64> {
    call(72, 0, 0).map(|_| ())
}

/// Replace one complete row. Each pixel stores red in bits 16–23, green in
/// bits 8–15 and blue in bits 0–7. A row outside the scanout returns -EINVAL.
pub fn write_row(y: usize, pixels: &[u32; WIDTH]) -> Result<(), i64> {
    call(73, y, pixels.as_ptr() as usize).map(|_| ())
}

/// Read one serial byte, or return -EAGAIN when no byte is waiting.
pub fn poll_key() -> Result<u8, i64> {
    call(74, 0, 0).map(|byte| byte as u8)
}

/// PL011 receive state since this guest boot. Each call samples one counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum RxMetric {
    IrqEntries = 0,
    /// Valid bytes read from the hardware, including bytes lost to a full ring.
    Bytes = 1,
    Drops = 2,
    /// UART data words discarded because the device reported an error.
    ErrorWords = 3,
    Depth = 4,
    Capacity = 5,
    /// Subset of IRQ entries taken while the kernel was idle.
    IdleIrqEntries = 6,
}

/// Query the selected display owner's IRQ receive driver. Polled profiles return -ENOSYS.
pub fn rx_stat(metric: RxMetric) -> Result<usize, i64> {
    call(75, metric as usize, 0)
}
