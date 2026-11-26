use crate::protocol::{Drawing, Id, BORDER, HEIGHT, MASK_HEIGHT, MASK_WIDTH, SCALE, TITLE_HEIGHT};
use jnox_native::display;

type Row = [u32; display::WIDTH];
const LIGHT: u32 = 0xe0eee9;

// The same 5x7, MSB-left cell convention as the native display demo.
fn glyph(ch: u8) -> [u8; 7] {
    match ch {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'N' => [17, 25, 25, 21, 19, 19, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        _ => [0; 7],
    }
}

fn label(row: &mut Row, y: usize, left: usize, top: usize, words: &[u8], color: u32) {
    if !(top..top + 7).contains(&y) { return; }
    for (i, &ch) in words.iter().enumerate() {
        let bits = glyph(ch)[y - top];
        for col in 0..5 {
            if bits & (16 >> col) != 0 { row[left + i * 6 + col] = color; }
        }
    }
}

fn window(row: &mut Row, screen_y: usize, drawing: &Drawing, focused: bool) {
    let (left, top, title): (usize, usize, &[u8]) = match drawing.id {
        Id::A => (24, 32, b"A  PULSE"),
        Id::B => (112, 84, b"B  TILES"),
    };
    if !(top..top + HEIGHT).contains(&screen_y) { return; }
    let width = drawing.id.width();
    let y = screen_y - top;
    let border = if focused { LIGHT } else { 0x536572 };
    row[left..left + width].fill(border);
    if !(BORDER..HEIGHT - BORDER).contains(&y) { return; }
    let inner = &mut row[left + BORDER..left + width - BORDER];
    if y < TITLE_HEIGHT {
        inner.fill(0x233541);
        label(row, y, left + 10, 8, title, LIGHT);
        if (9..15).contains(&y) { row[left + width - 16..left + width - 10].fill(border); }
        return;
    }
    inner.fill(drawing.body);
    let motif_top = usize::from(drawing.y);
    if (motif_top..motif_top + MASK_HEIGHT * SCALE).contains(&y) {
        let bits = drawing.mask[(y - motif_top) / SCALE];
        for col in 0..MASK_WIDTH {
            if bits & (0x8000 >> col) != 0 {
                let x = left + usize::from(drawing.x) + col * SCALE;
                row[x..x + SCALE].fill(drawing.ink);
            }
        }
    }
}

pub fn present(back: &Drawing, front: &Drawing) -> Result<(), i32> {
    for y in 0..display::HEIGHT {
        let mut row = [0x101923; display::WIDTH];
        label(&mut row, y, 24, 12, b"LOCAL WINDOWS", 0xacbdc5);
        label(&mut row, y, 24, 220, b"N FOCUS   X INK   Q QUIT", 0xacbdc5);
        window(&mut row, y, back, false);
        window(&mut row, y, front, true);
        display::write_row(y, &row).map_err(|e| e as i32)?;
    }
    Ok(())
}
