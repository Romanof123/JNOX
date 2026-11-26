//! QEMU virt RAMFB scanout and polled PL011 input. Calls are serialized at
//! masked EL1 on CPU0. Kernel BSS is identity mapped, Normal non-cacheable.
use crate::native_el0_mmu::FW_CFG;
use core::{arch::asm, mem::size_of, ptr};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const STRIDE: usize = WIDTH * size_of::<u32>();
const XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
const EIO: i64 = -5;
const ENODEV: i64 = -19;
const EINVAL: i64 = -22;
const ETIMEDOUT: i64 = -110;

#[repr(C, align(4096))]
struct Framebuffer([[u32; WIDTH]; HEIGHT]);

#[unsafe(no_mangle)]
static mut jnox_native_ramfb_pixels: Framebuffer = Framebuffer([[0; WIDTH]; HEIGHT]);

// QEMU hw/display/ramfb.c defines this 28-byte, big-endian fw_cfg file.
#[repr(C, packed)]
struct RamfbConfig {
    address: u64,
    fourcc: u32,
    flags: u32,
    width: u32,
    height: u32,
    stride: u32,
}

#[repr(C)]
struct DmaDescriptor {
    control: u32,
    length: u32,
    address: u64,
}

#[repr(C, align(16))]
struct Transfer {
    descriptor: DmaDescriptor,
    config: RamfbConfig,
}

const _: () = assert!(
    size_of::<RamfbConfig>() == 28
        && size_of::<DmaDescriptor>() == 16
        && size_of::<Framebuffer>() == 307_200
);

// Every field is integer data. Keep the payload and descriptor live even if
// completion times out and the device still owns them.
static mut TRANSFER: Transfer = unsafe { core::mem::zeroed() };
static mut INIT_RESULT: Option<Result<(), i64>> = None;

fn barrier() {
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
}

fn fw_select(selector: u16) {
    unsafe { ((FW_CFG + 8) as *mut u16).write_volatile(selector.to_be()) }
}

fn fw_bytes<const N: usize>() -> [u8; N] {
    core::array::from_fn(|_| unsafe { (FW_CFG as *const u8).read_volatile() })
}

fn ramfb_selector() -> Result<u16, i64> {
    fw_select(0); // FW_CFG_SIGNATURE
    if fw_bytes::<4>() != *b"QEMU" {
        return Err(ENODEV);
    }
    fw_select(1); // FW_CFG_ID is little-endian; bit 1 advertises DMA.
    if u32::from_le_bytes(fw_bytes()) & 2 == 0 {
        return Err(ENODEV);
    }
    fw_select(0x19); // FW_CFG_FILE_DIR
    let count = u32::from_be_bytes(fw_bytes());
    // Generic file selectors occupy [0x20, 0x4000).
    if count > 0x4000 - 0x20 {
        return Err(EIO);
    }
    for _ in 0..count {
        let size = u32::from_be_bytes(fw_bytes());
        let key = u16::from_be_bytes(fw_bytes());
        let _reserved = fw_bytes::<2>();
        if fw_bytes::<56>().starts_with(b"etc/ramfb\0") {
            if size != size_of::<RamfbConfig>() as u32 || !(0x20..0x4000).contains(&key) {
                return Err(EIO);
            }
            return Ok(key);
        }
    }
    Err(ENODEV)
}

fn configure() -> Result<(), i64> {
    let selector = ramfb_selector()?;
    let config = unsafe { &raw mut TRANSFER.config };
    let desc = unsafe { &raw mut TRANSFER.descriptor };
    const DMA_SELECT_WRITE: u32 = (1 << 3) | (1 << 4);
    unsafe {
        config.write(RamfbConfig {
            address: ((&raw const jnox_native_ramfb_pixels) as u64).to_be(),
            fourcc: XRGB8888.to_be(),
            flags: 0,
            width: (WIDTH as u32).to_be(),
            height: (HEIGHT as u32).to_be(),
            stride: (STRIDE as u32).to_be(),
        });
        desc.write(DmaDescriptor {
            control: ((u32::from(selector) << 16) | DMA_SELECT_WRITE).to_be(),
            length: (size_of::<RamfbConfig>() as u32).to_be(),
            address: (config as u64).to_be(),
        });
        barrier();
        ((FW_CFG + 16) as *mut u64).write_volatile((desc as u64).to_be());
        barrier();
        for _ in 0..65_536 {
            let control = u32::from_be((&raw const (*desc).control).read_volatile());
            if control == 0 {
                return Ok(());
            }
            if control & 1 != 0 {
                return Err(EIO);
            }
            core::hint::spin_loop();
        }
    }
    Err(ETIMEDOUT)
}

pub(crate) fn init() -> Result<(), i64> {
    // A mode is configured once per guest boot. Caching a terminal failure
    // also prevents a timed-out DMA descriptor from being reused.
    if let Some(result) = unsafe { INIT_RESULT } {
        return result;
    }
    let result = configure();
    unsafe { INIT_RESULT = Some(result) };
    result
}

pub(crate) fn write_row(y: usize, row: &[u32; WIDTH]) -> Result<(), i64> {
    if y >= HEIGHT {
        return Err(EINVAL);
    }
    unsafe {
        INIT_RESULT.unwrap_or(Err(ENODEV))?;
        let dst = (&raw mut jnox_native_ramfb_pixels.0[y]).cast::<u32>();
        ptr::copy_nonoverlapping(row.as_ptr(), dst, WIDTH);
    }
    barrier();
    Ok(())
}

pub(crate) fn poll_key() -> Result<u8, i64> {
    unsafe { INIT_RESULT.unwrap_or(Err(ENODEV))?; }
    crate::uart_try_read()
}
