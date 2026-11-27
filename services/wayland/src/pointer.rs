use crate::{client::{serial, Client}, platform, Scene, POS};

#[derive(Default)]
pub struct Pointer {
    pub available: bool,
    position: (Option<i32>, Option<i32>),
    pending: (Option<i32>, Option<i32>),
    buttons: [bool; 16],
    count: usize,
    held: bool,
    grab: Option<(usize, u64)>,
}

impl Pointer {
    pub fn new() -> Self {
        Self { available: platform::pointer_available(), ..Self::default() }
    }

    fn local(clients: &[Client; 2], i: usize, pos: (i32, i32)) -> (i64, i64) {
        let c = &clients[i];
        let anchor = POS[Scene::position(i, c)];
        let (left, top) = c.surface.buffer_origin(anchor);
        (i64::from(pos.0) - left + i64::from(c.surface.content_x),
         i64::from(pos.1) - top + i64::from(c.surface.content_y))
    }

    fn target(&self, scene: &Scene, clients: &[Client; 2]) -> Option<(usize, i32, i32)> {
        let pos = (self.position.0?, self.position.1?);
        let front = scene.front.unwrap_or(0);
        for i in [front, 1-front] {
            let c = &clients[i];
            if !Scene::visible(c) { continue; }
            let (x, y) = Self::local(clients, i, pos);
            let (w, h) = if c.surface.transform & 1 == 0 {
                (c.surface.width, c.surface.height)
            } else {
                (c.surface.height, c.surface.width)
            };
            let bx = x - i64::from(c.surface.content_x);
            let by = y - i64::from(c.surface.content_y);
            let inside = bx >= 0 && by >= 0 && bx < w.div_ceil(c.surface.scale) as i64
                && by < h.div_ceil(c.surface.scale) as i64 && c.input_contains(x, y);
            if self.held {
                if self.grab != Some((i, c.surface_generation)) { continue; }
            } else if !inside { continue; }
            let fixed = |v: i64| (v * 256).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            return Some((i, fixed(x), fixed(y)));
        }
        None
    }

    pub fn reconcile(&self, scene: &mut Scene, clients: &mut [Client; 2], time: u32) {
        if !self.available { return; }
        let target = self.target(scene, clients);
        for (i, c) in clients.iter_mut().enumerate() {
            if let Some((surface, _, _)) = c.pointer_local {
                if target.is_none_or(|p| p.0 != i) || c.surface.id != surface {
                    c.clear_pointer_focus();
                    if c.surface.id == surface && c.dying.is_none() {
                        if let Err(e) = c.pointer_event(1, &[serial(&mut scene.serial), surface]) {
                            c.fault(e, time);
                        }
                    }
                }
            }
        }
        if let Some((i, x, y)) = target {
            let c = &mut clients[i];
            let state = Some((c.surface.id, x, y));
            if state == c.pointer_local { return; }
            let result = if c.pointer_local.is_none() {
                c.pointer_event(0, &[serial(&mut scene.serial), c.surface.id, x as u32, y as u32])
            } else {
                c.pointer_event(2, &[time, x as u32, y as u32])
            };
            c.pointer_local = state;
            c.cursor_moved();
            if let Err(e) = result { c.fault(e, time); }
        }
    }

    fn lost(&mut self, scene: &mut Scene, clients: &mut [Client; 2], time: u32) {
        self.position = (None, None);
        self.reconcile(scene, clients, time);
        self.available = false;
        for c in clients.iter_mut() {
            if let Err(e) = c.pointer_lost() { c.fault(e, time); }
        }
    }

    pub fn poll(&mut self, scene: &mut Scene, clients: &mut [Client; 2], time: u32) -> bool {
        if !self.available { return false; }
        let mut progressed = false;
        for _ in 0..64 {
            let event = match platform::pointer_read() {
                Ok(value) => value,
                Err(-11) => break,
                Err(_) => { self.lost(scene, clients, time); break; }
            };
            progressed = true;
            let value = event as u32;
            match ((event >> 48) as u16, (event >> 32) as u16) {
                (3, 0) => self.pending.0 = Some(value.min(319) as i32),
                (3, 1) => self.pending.1 = Some(value.min(239) as i32),
                (1, 272) if value <= 1 => {
                    if self.count == self.buttons.len() { self.lost(scene, clients, time); break; }
                    self.buttons[self.count] = value == 1;
                    self.count += 1;
                }
                (0, 0) => {
                    self.position = self.pending;
                    self.reconcile(scene, clients, time);
                    for i in 0..self.count {
                        let pressed = self.buttons[i];
                        if self.held == pressed { continue; }
                        if let Some((slot, _, _)) = self.target(scene, clients) {
                            let c = &mut clients[slot];
                            if pressed { self.grab = Some((slot, c.surface_generation)); }
                            if let Err(e) = c.pointer_event(3, &[serial(&mut scene.serial), time, 272, u32::from(pressed)]) {
                                c.fault(e, time);
                            }
                        }
                        self.held = pressed;
                        if !pressed { self.grab = None; }
                        self.reconcile(scene, clients, time);
                    }
                    self.count = 0;
                }
                _ => {}
            }
        }
        progressed
    }
}
