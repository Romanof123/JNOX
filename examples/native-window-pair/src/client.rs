use crate::protocol::{Drawing, Id, Message, Stamp, PATH, PROTOCOL_ERROR};
use core::time::Duration;
use jnox_native::{display, local_socket as socket, monotonic_time, sleep_until};

fn content(id: Id, stamp: Stamp) -> Drawing {
    let changed = stamp.count % 2 == 1;
    match id {
        Id::A => Drawing {
            id, stamp, body: if changed { 0x398657 } else { 0x276575 }, ink: 0xd2ece8,
            x: 16, y: 36,
            mask: [0x0010, 0x0030, 0x0028, 0x0028, 0x0068, 0x0448, 0x0a44, 0xf244,
                   0x0182, 0x0102, 0x0101, 0x0000],
        },
        Id::B => Drawing {
            id, stamp, body: if changed { 0xa25639 } else { 0x704567 }, ink: 0xf1d5bc,
            x: 72, y: 36,
            mask: [0x7e7e, 0x4242, 0x5a5a, 0x5a5a, 0x4242, 0x7e7e, 0x0000, 0x3c3c,
                   0x2424, 0x2424, 0x3c3c, 0x0000],
        },
    }
}

fn connect(fd: i32) -> Result<(), i32> {
    // A client may reach the broker before the owner has bound and listened.
    // Sleep between retries so a fixed-priority peer can finish startup.
    let deadline = monotonic_time().map_err(|_| 91)? + Duration::from_secs(3);
    loop {
        match socket::connect(fd, PATH) {
            Err(-2 | -111) => {
                let now = monotonic_time().map_err(|_| 91)?;
                if now >= deadline { return Err(92); }
                sleep_until(now + Duration::from_millis(10)).map_err(|_| 91)?;
            }
            outcome => return outcome,
        }
    }
}

pub fn run(id: Id) -> Result<i32, i32> {
    let init = display::init();
    let row = display::write_row(0, &[0; display::WIDTH]);
    let key = display::poll_key();
    if (init, row, key) != (Err(-1), Err(-1), Err(-1)) { return Err(93); }

    let fd = socket::socket(socket::SOCK_STREAM, false)?;
    connect(fd)?;
    let mut seen = Stamp::default();
    content(id, seen).send(fd)?;
    loop {
        let msg = Message::receive(fd)?;
        if msg.id != id { return Err(PROTOCOL_ERROR); }
        match msg.op {
            b'x' => {
                let count = seen.count.checked_add(1).ok_or(PROTOCOL_ERROR)?;
                if msg.stamp.count != count || msg.stamp.serial <= seen.serial {
                    return Err(PROTOCOL_ERROR);
                }
                seen = Stamp { count, serial: msg.stamp.serial };
                content(id, seen).send(fd)?;
            }
            b'q' if msg.stamp == seen => {
                Message { op: b'k', id, stamp: seen }.send(fd)?;
                socket::close(fd)?;
                // Distinct exits attest the exact tested sequence. Other valid
                // interactive sequences still close both streams normally.
                return Ok(match (id, seen.count, seen.serial) {
                    (Id::A, 1, 1) => 71,
                    (Id::B, 1, 2) => 72,
                    _ => 0,
                });
            }
            _ => return Err(PROTOCOL_ERROR),
        }
    }
}
