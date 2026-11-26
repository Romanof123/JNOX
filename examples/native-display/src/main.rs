#![no_std]
#![no_main]

use core::{panic::PanicInfo, time::Duration};
use jnox_native::{display, exit, monotonic_time, sleep_until};

type Row = [u32; display::WIDTH];

// Five columns, most significant bit on the left, seven rows top to bottom.
fn glyph(ch: u8) -> [u8; 7] {
    match ch {
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'N' => [17, 25, 25, 21, 19, 19, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}

fn text(row: &mut Row, y: usize, left: usize, top: usize, scale: usize, label: &[u8], color: u32) {
    if !(top..top + 7 * scale).contains(&y) { return; }
    let glyph_y = (y - top) / scale;
    for (index, &ch) in label.iter().enumerate() {
        let mask = glyph(ch)[glyph_y];
        for column in 0..5 {
            if mask & (16 >> column) != 0 {
                let x = left + (index * 6 + column) * scale;
                row[x..x + scale].fill(color);
            }
        }
    }
}

fn draw(after: bool) -> Result<(), i64> {
    let accent = if after { 0x9fe6ab } else { 0x6ecce0 };
    for y in 0..display::HEIGHT {
        let mut row = [0x101923; display::WIDTH];
        if (44..190).contains(&y) {
            row[16..304].fill(if after { 0x23433d } else { 0x203747 });
            row[16..22].fill(accent);
        }
        if (204..206).contains(&y) { row[16..304].fill(0x526471); }
        text(&mut row, y, 16, 14, 2, b"J-NOX DISPLAY", 0xe8eff1);
        text(&mut row, y, 42, 62, 8, if after { b"02" } else { b"01" }, 0xe8eff1);
        text(&mut row, y, 42, 139, 3, if after { b"AFTER" } else { b"BEFORE" }, accent);
        text(&mut row, y, 16, 216, 2, b"N NEXT   Q QUIT", 0xacbdc5);
        display::write_row(y, &row)?;
    }
    Ok(())
}

fn denied_peer() -> i32 {
    if display::init() == Err(-1) && display::write_row(0, &[0; display::WIDTH]) == Err(-1)
        && display::poll_key() == Err(-1) { 61 } else { 60 }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc == 2 {
        let arg = *argv.add(1);
        // Kernel startup supplies NUL-terminated strings. A shorter argument
        // stops at its first NUL, and the complete match reads seven bytes.
        if b"denied\0".iter().enumerate().all(|(i, &byte)| arg.add(i).read_volatile() == byte) {
            return denied_peer();
        }
    }
    if argc != 1 { return 62; }
    match display::init() {
        Ok(()) => {}
        Err(-19) => return 19,
        Err(-38) => return 38,
        Err(_) => return 63,
    }
    if display::write_row(display::HEIGHT, &[0; display::WIDTH]) != Err(-22) { return 64; }
    if draw(false).is_err() { return 65; }
    let mut after = false;
    loop {
        match display::poll_key() {
            Ok(b'n') if !after => {
                if draw(true).is_err() { return 66; }
                after = true;
            }
            Ok(b'q') => return if after { 0 } else { 2 },
            Ok(_) => {}
            Err(-11) => {
                let Ok(now) = monotonic_time() else { return 67 };
                if sleep_until(now + Duration::from_millis(40)).is_err() { return 68; }
            }
            Err(_) => return 69,
        }
    }
}
