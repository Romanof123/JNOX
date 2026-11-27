#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

mod client;
mod input;
mod pointer;
mod observe;
#[cfg(target_os = "none")]
#[path = "guest.rs"]
mod platform;
#[cfg(not(target_os = "none"))]
#[path = "host.rs"]
mod platform;
mod surface;
mod wire;
use client::{serial, Client};
use surface::PIXELS;

const BACKGROUND: u32 = 0x18232d;
const POS: [(i64, i64); 2] = [(16, 24), (112, 80)];
static mut CLIENTS: [Client; 2] = [const { Client::new() }; 2];
static mut IMAGES: [[u32; PIXELS]; 2] = [[0; PIXELS]; 2];

struct Scene {
    front: Option<usize>,
    focus_surface: u32,
    serial: u32,
    first_pair: bool,
    presents: u64,
}
impl Scene {
    fn new() -> Self {
        Self {
            front: None,
            focus_surface: 0,
            serial: 0,
            first_pair: false,
            presents: 0,
        }
    }
    fn visible(c: &Client) -> bool {
        c.fd >= 0 && c.dying.is_none() && c.surface.mapped
    }
    fn position(i: usize, c: &Client) -> usize {
        match &c.surface.app_id[..c.surface.app_len] {
            b"org.jnox.Pulse" => 0,
            b"org.jnox.Tiles" => 1,
            _ => i,
        }
    }
    fn focus(&mut self, next: Option<usize>, clients: &mut [Client; 2], time: u32) {
        if self.front == next && self.focus_surface == next.map_or(0, |i| clients[i].surface.id) {
            return;
        }
        if let Some(i) = self.front {
            let c = &mut clients[i];
            // A deleted surface cannot be mentioned in an event. Other
            // resources belonging to a disconnected client have already gone.
            if c.fd >= 0 && c.surface.id == self.focus_surface && c.dying.is_none() {
                if let Err(err) =
                    c.keyboard_focus(false, self.focus_surface, serial(&mut self.serial))
                {
                    c.fault(err, time);
                }
            }
        }
        self.front = next;
        self.focus_surface = next.map_or(0, |i| clients[i].surface.id);
        if let Some(i) = next {
            if let Err(err) =
                clients[i].keyboard_focus(true, self.focus_surface, serial(&mut self.serial))
            {
                clients[i].fault(err, time);
            }
        }
    }
    fn reconcile(&mut self, clients: &mut [Client; 2], time: u32) {
        let online = clients.iter().filter(|c| Self::visible(c)).count();
        if online == 0 {
            self.first_pair = false;
        }
        let mut next = self.front.filter(|&i| Self::visible(&clients[i]));
        if next.is_none() {
            next = clients.iter().position(Self::visible);
        }
        if online == 2 && !self.first_pair {
            // The two packaged apps identify themselves over xdg_toplevel.
            // Generic peers retain focus on whichever surface mapped first.
            if let Some(i) = clients
                .iter()
                .position(|c| &c.surface.app_id[..c.surface.app_len] == b"org.jnox.Pulse")
            {
                next = Some(i);
            }
            self.first_pair = true;
        }
        self.focus(next, clients, time);
    }
    fn draw(
        &mut self,
        clients: &mut [Client; 2],
        images: &[[u32; PIXELS]; 2],
        time: u32,
    ) -> Result<(), i32> {
        let front = self.front.unwrap_or(0);
        for y in 0..240 {
            let mut row = [BACKGROUND; 320];
            for index in [1 - front, front] {
                let c = &clients[index];
                if !Self::visible(c) {
                    continue;
                }
                let s = &c.surface;
                let (left, top) = s.buffer_origin(POS[Self::position(index, c)]);
                let sy = y as i64 - top;
                if sy < 0 {
                    continue;
                }
                for (x, pixel) in row.iter_mut().enumerate() {
                    let sx = x as i64 - left;
                    if sx < 0 {
                        continue;
                    }
                    if let Some(color) = s.pixel(sx as usize, sy as usize, &images[index]) {
                        *pixel = surface::over(color, *pixel);
                    }
                }
            }
            platform::row(y, &row).map_err(|_| 93)?;
        }
        self.presents += 1;
        let mut receipt = [0u64; 16];
        receipt[..4].copy_from_slice(&[
            self.presents,
            time.into(),
            self.front.map_or(u64::MAX, |i| i as u64),
            self.serial.into(),
        ]);
        for (i, c) in clients.iter_mut().enumerate() {
            let offset = 4 + 6 * i;
            let s = &c.surface;
            receipt[offset..offset + 6].copy_from_slice(&[
                c.fd as i64 as u64,
                s.id.into(),
                s.top.into(),
                u64::from(Self::visible(c)),
                s.width as u64,
                s.height as u64,
            ]);
            c.changed = false;
            if c.fd >= 0 && c.dying.is_none() {
                if let Err(e) = c.presented(time) {
                    c.fault(e, time);
                }
            }
        }
        unsafe {
            (&raw mut observe::jnox_wayland_scene).write_volatile(receipt);
        }
        Ok(())
    }
}

// Bound each peer's work per iteration so a split header, a full TX queue or
// an unterminated handshake cannot block keyboard or another connection.
fn service(
    c: &mut Client,
    image: &mut [u32; PIXELS],
    last: &mut u32,
    focused: bool,
    time: u32,
) -> bool {
    if c.fd < 0 {
        return false;
    }
    let mut progressed = false;
    match c.out.flush(c.fd) {
        Ok(wrote) => progressed |= wrote,
        Err(-11) => {}
        Err(_) => {
            c.clear();
            return true;
        }
    }
    if let Some(start) = c.dying {
        if c.out.is_empty() || time.wrapping_sub(start) > 1000 {
            c.clear();
            return true;
        }
        return progressed;
    }
    for _ in 0..8 {
        match c.step(last, focused, image) {
            Ok(true) => {
                progressed = true;
                continue;
            }
            Err(err) => {
                c.fault(err, time);
                c.changed = true;
                return true;
            }
            Ok(false) => {}
        }
        match c.incoming.receive(c.fd) {
            Ok(0) => {
                c.clear();
                return true;
            }
            Ok(_) => progressed = true,
            Err(-11) => break,
            Err(_) => {
                c.clear();
                return true;
            }
        }
    }
    progressed
}

fn run() -> Result<(), i32> {
    let listener = platform::listen()?;
    let (clients, images) = unsafe { (&mut *(&raw mut CLIENTS), &mut *(&raw mut IMAGES)) };
    let mut scene = Scene::new();
    let mut pointer = pointer::Pointer::new();
    let mut closing = None;
    scene.draw(clients, images, platform::now_ms())?;
    loop {
        let time = platform::now_ms();
        let mut progressed = false;
        if closing.is_none() {
            if let Some(slot) = clients.iter_mut().find(|c| c.fd < 0) {
                match platform::accept(listener) {
                    Ok(fd) => {
                        slot.start(fd);
                        slot.pointer_available = pointer.available;
                        progressed = true;
                    }
                    Err(-11) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        for i in 0..2 {
            progressed |= service(
                &mut clients[i],
                &mut images[i],
                &mut scene.serial,
                scene.front == Some(i),
                time,
            );
        }
        let old = scene.front;
        scene.reconcile(clients, time);
        if let Some(byte) = platform::input() {
            progressed = true;
            match byte {
                b'n' => {
                    if let Some(i) = scene.front {
                        if Scene::visible(&clients[1 - i]) {
                            scene.focus(Some(1 - i), clients, time);
                        }
                    }
                }
                b'q' if closing.is_none() => {
                    closing = Some(time);
                    for c in clients.iter_mut().filter(|c| c.fd >= 0) {
                        if let Err(e) = c.close_event() {
                            c.fault(e, time);
                        }
                    }
                }
                b => {
                    if let (Some(i), Some(code)) = (scene.front, input::keycode(b)) {
                        for pressed in [true, false] {
                            if let Err(e) =
                                clients[i].key(code, pressed, time, serial(&mut scene.serial))
                            {
                                clients[i].fault(e, time);
                                break;
                            }
                        }
                    }
                }
            }
        }
        pointer.reconcile(&mut scene, clients, time);
        progressed |= pointer.poll(&mut scene, clients, time);
        if scene.front != old || clients.iter().any(|c| c.changed) {
            scene.draw(clients, images, time)?;
        }
        if let Some(start) = closing {
            if clients.iter().all(|c| c.fd < 0) || time.wrapping_sub(start) > 5000 {
                break;
            }
        }
        if !progressed {
            platform::idle();
        }
    }
    for c in clients {
        c.clear();
    }
    platform::close(listener);
    Ok(())
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    jnox_native::exit(111)
}
#[cfg(target_os = "none")]
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    run().err().unwrap_or(0)
}
#[cfg(not(target_os = "none"))]
fn main() {
    if let Err(e) = run() {
        eprintln!("Wayland compositor failed ({e})");
        std::process::exit(1);
    }
}
