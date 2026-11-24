//! One trusted, statically packaged native-v1 process on a dedicated virt boot.
use crate::native_elf::{self, Permission};
use crate::native_v1_note::native_v1_note;
use crate::native_el0_mmu::{self, STACK_BOTTOM, STACK_TOP};
use crate::{park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

global_asm!(include_str!("native_el0_vectors.S"), options(raw));
unsafe extern "C" {
    fn native_el0_enter(entry: u64, sp: u64) -> !;
    static native_el0_vectors: u8;
}

const ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));
#[used]
#[unsafe(no_mangle)]
static jnox_native_el0_image: [u8; ELF.len()] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));

#[repr(C, align(16))]
struct NativeFrame {
    x: [u64; 31],
    esr: u64,
    spsr: u64,
    elr: u64,
    sp_el0: u64,
    current_el: u64,
    fpcr: u64,
    fpsr: u64,
    q: [[u64; 2]; 32],
}
const _: () = assert!(size_of::<NativeFrame>() == 816
    && offset_of!(NativeFrame, esr) == 248 && offset_of!(NativeFrame, spsr) == 256
    && offset_of!(NativeFrame, elr) == 264 && offset_of!(NativeFrame, sp_el0) == 272
    && offset_of!(NativeFrame, current_el) == 280 && offset_of!(NativeFrame, q) == 304);

#[repr(C)]
struct Journal {
    header: [u64; 16],
    rows: [[u64; 16]; 8],
}
#[unsafe(no_mangle)]
static mut jnox_native_el0_journal: Journal = Journal { header: [0; 16], rows: [[0; 16]; 8] };

fn head(index: usize, value: u64) {
    // One CPU. Boot and the EL1 vectors are masked and never nested.
    unsafe { ptr::write_volatile((&raw mut jnox_native_el0_journal.header).cast::<u64>().add(index), value) };
}

fn decimal(value: i64) {
    if value < 0 {
        uart_write(b"-");
    }
    let mut n = value.unsigned_abs();
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&digits[i..]);
}

fn fail(code: u64) -> ! {
    head(2, code);
    head(1, 4);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    uart_write(b"NATIVE EL0 FAULT ");
    decimal(code as i64);
    uart_write(b"\n");
    park()
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_unexpected() -> ! {
    fail(1)
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_sync(frame: *const NativeFrame, vector: u64) -> i64 {
    // The lower-A64 vector created this private, 16-byte aligned EL1 frame.
    let f = unsafe { &*frame };
    let vbar: u64;
    let ttbr: u64;
    let far: u64;
    unsafe {
        asm!("mrs {}, vbar_el1", "mrs {}, ttbr0_el1", "mrs {}, far_el1",
            out(reg) vbar, out(reg) ttbr, out(reg) far, options(nostack, preserves_flags));
    }
    let count = unsafe { ptr::read_volatile(&raw const jnox_native_el0_journal.header[8]) as usize };
    if count >= 8 || vector != 0x400 || vbar != (&raw const native_el0_vectors).addr() as u64
        || f.current_el != 4 || f.spsr & 0x1f != 0 || f.esr >> 26 != 0x15
        || f.esr & 0xffff != 0x4a01 || !native_el0_mmu::user_range(f.elr as usize, 4, false)
        || !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&f.sp_el0) || f.sp_el0 % 16 != 0
    {
        fail(2);
    }
    let exit = f.x[8] == 1;
    if exit && f.x[0] as i32 as i64 as u64 != f.x[0] {
        fail(3);
    }
    // Unsupported services do not dereference a user pointer, acquire
    // descriptors, emit output or claim to have completed a transfer.
    let result = if exit { 0i64 } else { -38 };
    let row = [count as u64, vector, f.current_el, f.esr, f.spsr, f.elr, f.sp_el0,
        f.x[8], f.x[0], f.x[1], f.x[2], result as u64, if exit { 2 } else { 1 }, vbar, ttbr, far];
    unsafe {
        let out = (&raw mut jnox_native_el0_journal.rows).cast::<[u64; 16]>().add(count);
        ptr::write_volatile(out, row);
        asm!("dmb ishst", options(nostack, preserves_flags));
    }
    head(8, count as u64 + 1);
    if exit {
        head(9, f.x[0]);
        head(10, 1);
        head(1, 3);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
        uart_write(b"NATIVE EL0 EXIT ");
        decimal(f.x[0] as i64);
        uart_write(b"\n");
        park()
    }
    result
}

// The bootstrap source is fixed. The ordinary argument strings and pointers
// are copied to private physical backing before the user mapping is enabled.
fn initial_stack() -> [u8; 80] {
    let args: [&[u8]; 3] = [b"/bin/native_startup_probe\0", b"8\0", b"13\0"];
    let mut tail = [0u8; 80];
    let sp = STACK_TOP - tail.len();
    tail[..8].copy_from_slice(&3u64.to_le_bytes());
    let mut offset = 48;
    for (i, arg) in args.iter().enumerate() {
        let pointer = ((sp + offset) as u64).to_le_bytes();
        tail[8 * (i + 1)..8 * (i + 2)].copy_from_slice(&pointer);
        tail[offset..offset + arg.len()].copy_from_slice(arg);
        offset += arg.len();
    }
    tail
}

pub(super) fn run() -> ! {
    head(0, 1);
    head(1, 1);
    head(3, jnox_native_el0_image.len() as u64);
    uart_write(b"NATIVE EL0 ELF ");
    uart_write(env!("JNOX_PACKAGED_ELF_SHA256").as_bytes());
    uart_write(b"\n");
    let plan = native_elf::plan(&jnox_native_el0_image, 0x0040_0000..0x0080_0000)
        .unwrap_or_else(|_| fail(4));
    let perms = [Permission::ReadExecute, Permission::Read, Permission::ReadWrite];
    if plan.entry != 0x0040_0000 || !plan.segments().iter().map(|s| s.permission).eq(perms)
        || native_v1_note(&jnox_native_el0_image, &plan).is_none()
    {
        fail(5);
    }
    let tail = initial_stack();
    let sp = STACK_TOP - tail.len();
    // Dedicated virt boot with a quiescent single CPU. Other profiles keep
    // their own interrupt and timer setup.
    unsafe {
        asm!("msr daifset, #0xf", "msr cntp_ctl_el0, xzr", "msr cntv_ctl_el0, xzr",
            "isb", options(nostack, preserves_flags));
        ptr::write_volatile(0x0801_0000 as *mut u32, 0);
        ptr::write_volatile(0x0800_0000 as *mut u32, 0);
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
        native_el0_mmu::install(&plan, &tail).unwrap_or_else(|_| fail(6));
    }
    let current: u64;
    let sctlr: u64;
    let ttbr: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", "mrs {}, sctlr_el1", "mrs {}, ttbr0_el1",
            out(reg) current, out(reg) sctlr, out(reg) ttbr, options(nostack, preserves_flags));
    }
    if current != 4 || sctlr & 1 == 0 || !native_el0_mmu::user_range(sp, tail.len(), true) {
        fail(7);
    }
    head(4, plan.entry);
    head(5, sp as u64);
    head(6, STACK_BOTTOM as u64);
    head(7, STACK_TOP as u64);
    head(12, current);
    head(13, sctlr);
    head(14, ttbr);
    head(15, 3);
    head(1, 2);
    unsafe { native_el0_enter(plan.entry, sp as u64) }
}
