#![no_std]
#![no_main]

mod client;
mod protocol;
mod scene;

use core::{panic::PanicInfo, time::Duration};
use jnox_native::{display, exit, local_socket as socket, monotonic_time, sleep_until};
use protocol::{Drawing, Id, Message, Stamp, PATH, PROTOCOL_ERROR};

struct Window {
    fd: i32,
    drawing: Drawing,
}

impl Window {
    fn accept(listener: i32) -> Result<Self, i32> {
        let fd = socket::accept(listener)?;
        let drawing = Drawing::receive(fd)?;
        if drawing.stamp != Stamp::default() { return Err(PROTOCOL_ERROR); }
        Ok(Self { fd, drawing })
    }

    fn key(&mut self, serial: u16) -> Result<(), i32> {
        let id = self.drawing.id;
        let count = self.drawing.stamp.count.checked_add(1).ok_or(PROTOCOL_ERROR)?;
        let stamp = Stamp { count, serial };
        Message { op: b'x', id, stamp }.send(self.fd)?;
        let drawing = Drawing::receive(self.fd)?;
        if drawing.id != id || drawing.stamp != stamp { return Err(PROTOCOL_ERROR); }
        self.drawing = drawing;
        Ok(())
    }

    fn close(self) -> Result<(), i32> {
        let quit = Message { op: b'q', id: self.drawing.id, stamp: self.drawing.stamp };
        quit.send(self.fd)?;
        let expected = Message { op: b'k', ..quit };
        if Message::receive(self.fd)? != expected { return Err(PROTOCOL_ERROR); }
        socket::close(self.fd)
    }
}

fn compose() -> Result<i32, i32> {
    let listener = socket::socket(socket::SOCK_STREAM, false)?;
    socket::bind(listener, PATH)?;
    socket::listen(listener, 2)?;
    let first = Window::accept(listener)?;
    let second = Window::accept(listener)?;
    socket::close(listener)?;
    let (mut front, mut back) = match (first.drawing.id, second.drawing.id) {
        (Id::A, Id::B) => (first, second),
        (Id::B, Id::A) => (second, first),
        _ => return Err(PROTOCOL_ERROR),
    };

    display::init().map_err(|e| e as i32)?;
    scene::present(&back.drawing, &front.drawing)?;
    let mut serial = 0u16;
    loop {
        match display::poll_key() {
            Ok(b'n') => core::mem::swap(&mut front, &mut back),
            Ok(b'x') => {
                serial = serial.checked_add(1).ok_or(PROTOCOL_ERROR)?;
                front.key(serial)?;
            }
            Ok(b'q') => {
                front.close()?;
                back.close()?;
                return Ok(70);
            }
            Ok(_) => continue,
            Err(-11) => {
                let now = monotonic_time().map_err(|_| 91)?;
                sleep_until(now + Duration::from_millis(20)).map_err(|_| 91)?;
                continue;
            }
            Err(errno) => return Err(errno as i32),
        }
        scene::present(&back.drawing, &front.drawing)?;
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if let Err(errno) = socket::set_broker(3) { return errno; }
    let result = match argc {
        1 => compose(),
        2 if !argv.is_null() => {
            let arg = *argv.add(1);
            if arg.is_null() { return 94; }
            // Native startup supplies a NUL-terminated string for every entry.
            let id = match *arg {
                b'a' if *arg.add(1) == 0 => Id::A,
                b'b' if *arg.add(1) == 0 => Id::B,
                _ => return 94,
            };
            client::run(id)
        }
        _ => return 94,
    };
    result.unwrap_or_else(|errno| errno)
}
