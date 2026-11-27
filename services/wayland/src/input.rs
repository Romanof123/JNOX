//! wl_seat and wl_keyboard v1–v4 events for the native serial keyboard.
use crate::{platform, wire::Out};

// wl_keyboard.key reports evdev codes. XKB's corresponding codes are offset
// by eight. The map is self-contained, including on clients without xkeyboard-config.
const KEYMAP: &[u8] = concat!(
    r#"xkb_keymap {
    xkb_keycodes "jnox" {
        minimum = 8;
        maximum = 255;
        <AC08> = 45;
        <AB02> = 53;
    };
    xkb_types "jnox" {
        type "ONE_LEVEL" {
            modifiers = None;
            level_name[Level1] = "Any";
        };
    };
    xkb_compatibility "jnox" {
    };
    xkb_symbols "jnox" {
        key <AC08> { type = "ONE_LEVEL", [ k ] };
        key <AB02> { type = "ONE_LEVEL", [ x ] };
    };
};
"#,
    "\0"
).as_bytes();

pub(crate) fn capabilities(out: &mut Out, id: u32, version: u32) -> Result<(), ()> {
    const KEYBOARD: u32 = 2;
    out.event(id, 0, &KEYBOARD.to_le_bytes())?;
    if version >= 2 {
        // wl_seat.name is a length-prefixed string including its NUL, padded to four bytes.
        out.event(id, 1, b"\x06\0\0\0seat0\0\0\0")?;
    }
    Ok(())
}

pub(crate) fn keymap(out: &mut Out, id: u32, version: u32) -> Result<(), ()> {
    const XKB_V1: u32 = 1;
    let fd = platform::keymap_fd(KEYMAP)?;
    // The FD travels in SCM_RIGHTS; it has no bytes in the event body.
    // event_fd takes ownership even if enqueueing fails.
    let body = [XKB_V1.to_le_bytes(), (KEYMAP.len() as u32).to_le_bytes()];
    out.event_fd(id, 0, body.as_flattened(), fd)?;
    if version >= 4 {
        out.event(id, 5, &[0; 8])?; // wl_keyboard.repeat_info(rate=0, delay=0)
    }
    Ok(())
}

pub(crate) fn enter(out: &mut Out, id: u32, serial: u32, surface: u32) -> Result<(), ()> {
    // Each UART byte produces a complete press/release before the next focus action.
    let body = [serial, surface, 0].map(u32::to_le_bytes); // empty pressed-keys array
    out.event(id, 1, body.as_flattened())?;
    let modifiers = [serial, 0, 0, 0, 0].map(u32::to_le_bytes);
    out.event(id, 4, modifiers.as_flattened())
}

pub(crate) fn leave(out: &mut Out, id: u32, serial: u32, surface: u32) -> Result<(), ()> {
    let body = [serial, surface].map(u32::to_le_bytes);
    out.event(id, 2, body.as_flattened())
}

pub(crate) fn keycode(byte: u8) -> Option<u32> {
    match byte {
        b'k' => Some(37),
        b'x' => Some(45),
        _ => None,
    }
}

pub(crate) fn key(
    out: &mut Out, id: u32, serial: u32, time_ms: u32, keycode: u32, pressed: bool,
) -> Result<(), ()> {
    let body = [serial, time_ms, keycode, u32::from(pressed)].map(u32::to_le_bytes);
    out.event(id, 3, body.as_flattened())
}
