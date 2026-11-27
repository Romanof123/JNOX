use crate::{
    input, platform,
    surface::{self, Attachment, Buffer, Geometry, Surface, PIXELS},
    wire::{with_string, In, Out, Reader, MAX_MESSAGE},
};

#[derive(Clone, Copy)]
enum Kind {
    Empty,
    Display,
    Registry,
    Compositor,
    Shm,
    Wm,
    Seat,
    Output { entered: bool },
    Keyboard,
    Pointer,
    Surface,
    Region(usize),
    Pool(usize),
    Buffer(Buffer),
    Xdg,
    Top,
    Callback,
    Frame,
    Presented,
    Positioner,
}
#[derive(Clone, Copy)]
struct Object {
    id: u32,
    kind: Kind,
    version: u32,
}
const EMPTY: Object = Object {
    id: 0,
    kind: Kind::Empty,
    version: 0,
};
#[derive(Clone, Copy)]
struct Region {
    ops: [(bool, [i32; 4]); 16],
    len: usize,
}
impl Region {
    const EMPTY: Self = Self { ops: [(false, [0; 4]); 16], len: 0 };
    fn contains(&self, x: i64, y: i64) -> bool {
        self.ops[..self.len].iter().rev().find_map(|&(add, [left, top, w, h])| {
            let dx = x - i64::from(left);
            let dy = y - i64::from(top);
            (dx >= 0 && dy >= 0 && dx < i64::from(w) && dy < i64::from(h)).then_some(add)
        }).unwrap_or(false)
    }
}

struct Pool {
    map: Option<platform::Mapping>,
    size: usize,
}

#[derive(Clone, Copy)]
pub struct Fault(pub u32, pub u32, pub &'static [u8]);
impl From<()> for Fault {
    fn from(_: ()) -> Self {
        Self(1, 1, b"invalid request")
    }
}
type Result<T> = core::result::Result<T, Fault>;
pub fn serial(last: &mut u32) -> u32 {
    *last = last.wrapping_add(1).max(1);
    *last
}

pub struct Client {
    pub fd: i32,
    pub incoming: In,
    pub out: Out,
    pub surface: Surface,
    objects: [Object; 64],
    pools: [Pool; 4],
    regions: [Region; 4],
    input_region: Option<Region>,
    pending_region: Option<Option<Region>>,
    configures: [u32; 8],
    configs: usize,
    pub dying: Option<u32>,
    pub focus_stall: Option<u32>,
    pub changed: bool,
    pub surface_generation: u64,
    pub pointer_available: bool,
    pub pointer_local: Option<(u32, i32, i32)>,
}
impl Client {
    pub const fn new() -> Self {
        Self {
            fd: -1,
            incoming: In::new(),
            out: Out::new(),
            surface: Surface::new(),
            objects: [EMPTY; 64],
            pools: [const { Pool { map: None, size: 0 } }; 4],
            regions: [Region::EMPTY; 4],
            input_region: None,
            pending_region: None,
            configures: [0; 8],
            configs: 0,
            dying: None,
            focus_stall: None,
            changed: false,
            surface_generation: 0,
            pointer_available: false,
            pointer_local: None,
        }
    }
    pub fn start(&mut self, fd: i32) {
        crate::observe::wire(fd, 3, &[], &[]);
        self.fd = fd;
        self.objects[0] = Object {
            id: 1,
            kind: Kind::Display,
            version: 1,
        };
    }
    pub fn clear(&mut self) {
        self.pointer_local = None;
        self.incoming.clear();
        self.out.clear();
        self.objects.fill(EMPTY);
        for p in &mut self.pools {
            p.map = None;
            p.size = 0;
        }
        self.surface = Surface::new();
        self.input_region = None;
        self.pending_region = None;
        self.configs = 0;
        self.dying = None;
        self.focus_stall = None;
        self.changed = true;
        if self.fd >= 0 {
            crate::observe::wire(self.fd, 4, &[], &[]);
            platform::close(self.fd);
            self.fd = -1;
        }
    }
    fn index(&self, id: u32) -> Result<usize> {
        if id != 0 {
            if let Some(i) = self.objects.iter().position(|o| o.id == id) {
                return Ok(i);
            }
        }
        Err(Fault(1, 0, b"unknown object ID"))
    }
    fn add(&mut self, id: u32, kind: Kind, version: u32) -> Result<()> {
        if id == 0 || id >= 0xff00_0000 || self.index(id).is_ok() {
            return Err(Fault(1, 0, b"invalid new object ID"));
        }
        let slot =
            self.objects
                .iter_mut()
                .find(|o| o.id == 0)
                .ok_or(Fault(1, 2, b"object table full"))?;
        *slot = Object { id, kind, version };
        Ok(())
    }
    fn delete(&mut self, id: u32) -> Result<()> {
        let i = self.index(id)?;
        self.objects[i] = EMPTY;
        self.out.words(1, 1, &[id])?;
        Ok(())
    }
    fn collect(&mut self) {
        for (index, pool) in self.pools.iter_mut().enumerate() {
            let referenced = self.objects.iter().any(|o| match o.kind {
                Kind::Pool(i) => i == index,
                Kind::Buffer(b) => b.pool == index,
                _ => false,
            }) || self
                .surface
                .pending
                .flatten()
                .is_some_and(|a| a.buffer.pool == index);
            if !referenced {
                pool.map = None;
                pool.size = 0;
            }
        }
    }
    fn configure(&mut self, last: &mut u32) -> Result<()> {
        if self.configs == self.configures.len() {
            return Err(Fault(1, 2, b"configure queue full"));
        }
        self.out.words(self.surface.top, 0, &[0, 0, 0])?; // compositor lets client choose its size
        let s = serial(last);
        self.out.words(self.surface.xdg, 0, &[s])?;
        self.surface.configure = s;
        self.configures[self.configs] = s;
        self.configs += 1;
        Ok(())
    }
    pub fn keyboard_focus(&mut self, enter: bool, surface: u32, serial: u32) -> Result<()> {
        for o in self.objects {
            if matches!(o.kind, Kind::Keyboard) {
                if enter {
                    input::enter(&mut self.out, o.id, serial, surface)?;
                } else {
                    input::leave(&mut self.out, o.id, serial, surface)?;
                }
            }
        }
        Ok(())
    }
    pub fn key(&mut self, code: u32, pressed: bool, time: u32, seq: u32) -> Result<()> {
        for o in self.objects {
            if matches!(o.kind, Kind::Keyboard) {
                input::key(&mut self.out, o.id, seq, time, code, pressed)?;
            }
        }
        Ok(())
    }
    pub fn input_contains(&self, x: i64, y: i64) -> bool {
        self.input_region.is_none_or(|r| r.contains(x, y))
    }
    pub fn pointer_event(&mut self, opcode: u16, words: &[u32]) -> Result<()> {
        for obj in self.objects.iter().filter(|o| matches!(o.kind, Kind::Pointer)) {
            if opcode <= 1 {
                self.out.focus_words(obj.id, opcode, words)?;
            } else {
                self.out.words(obj.id, opcode, words)?;
            }
        }
        Ok(())
    }
    pub fn pointer_lost(&mut self) -> Result<()> {
        self.pointer_available = false;
        for obj in self.objects.iter().filter(|o| matches!(o.kind, Kind::Seat)) {
            self.out.words(obj.id, 0, &[2])?;
        }
        Ok(())
    }
    pub fn close_event(&mut self) -> Result<()> {
        if self.surface.top != 0 {
            self.out.event(self.surface.top, 1, &[])?;
        }
        Ok(())
    }
    pub fn fault(&mut self, Fault(object, code, msg): Fault, now: u32) {
        let mut body = [0; 112];
        body[..4].copy_from_slice(&object.to_ne_bytes());
        body[4..8].copy_from_slice(&code.to_ne_bytes());
        let size = with_string(&mut body[8..], msg);
        if self.out.event(1, 0, &body[..8 + size]).is_err() {
            self.clear();
        } else {
            self.dying = Some(now);
        }
    }
    fn globals(&mut self, reg: u32) -> Result<()> {
        for (name, iface, ver) in GLOBALS {
            let mut body = [0; 48];
            body[..4].copy_from_slice(&name.to_ne_bytes());
            let end = 4 + with_string(&mut body[4..], iface);
            body[end..end + 4].copy_from_slice(&ver.to_ne_bytes());
            self.out.event(reg, 0, &body[..end + 4])?;
        }
        Ok(())
    }
    fn output_description(&mut self, id: u32, version: u32) -> Result<()> {
        // Origin, physical millimeters and subpixel layout are all zero for RAMFB.
        let mut body = [0u8; 64];
        let mut end = 20;
        end += with_string(&mut body[end..], b"JNOX");
        end += with_string(&mut body[end..], b"RAM framebuffer");
        self.out.event(id, 0, &body[..end+4])?; // normal transform
        self.out.words(id, 1, &[3, surface::WIDTH as u32, surface::HEIGHT as u32, 0])?;
        if version >= 2 {
            self.out.words(id, 3, &[1])?; // scale
            self.out.event(id, 2, &[])?; // done
        }
        Ok(())
    }
    fn output_membership(&mut self, slot: usize) -> Result<()> {
        let anchor = crate::POS[crate::Scene::position(slot, self)];
        let overlaps = self.surface.overlaps_output(anchor);
        for obj in &mut self.objects {
            if let Kind::Output { entered } = &mut obj.kind {
                if *entered != overlaps {
                    let opcode = if overlaps { 0 } else { 1 };
                    self.out.words(self.surface.id, opcode, &[obj.id])?;
                    *entered = overlaps;
                }
            }
        }
        Ok(())
    }
    fn commit(&mut self, last: &mut u32, image: &mut [u32; PIXELS]) -> Result<()> {
        if self.surface.xdg != 0 {
            if self.surface.top == 0 {
                return Err(Fault(self.surface.xdg, 1, b"missing surface role"));
            }
            if self.surface.pending.flatten().is_some() && !self.surface.acked {
                return Err(Fault(
                    self.surface.xdg,
                    3,
                    b"buffer committed before configure acknowledgement",
                ));
            }
            if self.surface.configure == 0 {
                self.configure(last)?;
            }
        }
        let s = &mut self.surface;
        s.scale = s.next_scale;
        s.transform = s.next_transform;
        if let Some(attachment) = s.pending.take() {
            if let Some(a) = attachment {
                let b = a.buffer;
                surface::snapshot(b, self.pools[b.pool].map.as_ref().ok_or(())?, image)
                    .map_err(|_| Fault(a.id, 2, b"shared backing extent changed"))?;
                s.width = b.width;
                s.height = b.height;
                s.content_x = s.content_x.saturating_add(s.dx);
                s.content_y = s.content_y.saturating_add(s.dy);
                s.mapped = s.top != 0;
                if a.id != 0 {
                    self.out.event(a.id, 0, &[])?;
                }
            } else {
                if s.mapped {
                    s.geometry = None;
                    s.next_geometry = None;
                    s.configure = 0;
                    s.acked = false;
                    self.configs = 0;
                }
                s.mapped = false;
                s.width = 0;
                s.height = 0;
                s.content_x = 0;
                s.content_y = 0;
            }
            s.dx = 0;
            s.dy = 0;
        }
        if s.width % s.scale != 0 || s.height % s.scale != 0 {
            return Err(Fault(
                s.id,
                2,
                b"buffer size is not divisible by surface scale",
            ));
        }
        s.commit_geometry();
        for o in &mut self.objects {
            if matches!(o.kind, Kind::Frame) {
                o.kind = Kind::Presented;
            }
        }
        s.damaged = false;
        self.changed = true;
        self.collect();
        Ok(())
    }
    pub fn presented(&mut self, time: u32) -> Result<()> {
        for i in 0..self.objects.len() {
            if matches!(self.objects[i].kind, Kind::Presented) {
                let id = self.objects[i].id;
                self.out.words(id, 0, &[time])?;
                self.delete(id)?;
            }
        }
        Ok(())
    }
    /// Consume at most one complete request. FDs and bytes are independent
    /// ordered streams; create_pool waits until both arguments have arrived.
    pub fn step(
        &mut self,
        last: &mut u32,
        focused: bool,
        image: &mut [u32; PIXELS],
        slot: usize,
    ) -> Result<bool> {
        let Some((id, op, size)) = self.incoming.header()? else {
            return Ok(false);
        };
        let obj = self.objects[self.index(id)?];
        let fd = if matches!(obj.kind, Kind::Shm) && op == 0 {
            let Some(fd) = self.incoming.pop_fd() else {
                return Ok(false);
            };
            Some(fd)
        } else {
            None
        };
        let mut bytes = [0; MAX_MESSAGE];
        self.incoming.consume(size, &mut bytes[..size - 8]);
        let result = self.dispatch(obj, op, &bytes[..size - 8], fd, last, focused, image);
        if let Some(fd) = fd {
            platform::close(fd);
        }
        result?;
        if matches!((obj.kind, op), (Kind::Registry, 0) | (Kind::Surface, 6) | (Kind::Top, 0 | 3)) {
            self.output_membership(slot)?;
        }
        Ok(true)
    }

    fn dispatch(
        &mut self,
        obj: Object,
        op: u16,
        bytes: &[u8],
        fd: Option<i32>,
        last: &mut u32,
        focused: bool,
        image: &mut [u32; PIXELS],
    ) -> Result<()> {
        let (id, v) = (obj.id, obj.version);
        let mut r = Reader::new(bytes);
        // Every branch consumes its complete signature before publishing it.
        match (obj.kind, op) {
            (Kind::Display, 0) => {
                let cb = r.u()?;
                r.end()?;
                self.add(cb, Kind::Callback, 1)?;
                self.out.words(cb, 0, &[serial(last)])?;
                self.delete(cb)?;
            }
            (Kind::Display, 1) => {
                let reg = r.u()?;
                r.end()?;
                self.add(reg, Kind::Registry, 1)?;
                self.globals(reg)?;
            }
            (Kind::Registry, 0) => {
                let (name, iface, version, new) = (r.u()?, r.string()?, r.u()?, r.u()?);
                r.end()?;
                let (_, _, max) = GLOBALS
                    .iter()
                    .find(|(n, i, _)| *n == name && *i == iface.as_bytes())
                    .ok_or(Fault(1, 0, b"unknown global"))?;
                if version == 0 || version > *max {
                    return Err(Fault(1, 0, b"invalid global version"));
                }
                let kind = match name {
                    1 => Kind::Compositor,
                    2 => Kind::Shm,
                    3 => Kind::Wm,
                    4 => Kind::Seat,
                    5 => Kind::Output { entered: false },
                    _ => unreachable!(),
                };
                self.add(new, kind, version)?;
                if name == 2 {
                    self.out.words(new, 0, &[0])?;
                    self.out.words(new, 0, &[1])?;
                }
                if name == 4 {
                    input::capabilities(&mut self.out, new, version, self.pointer_available)?;
                }
                if name == 5 {
                    self.output_description(new, version)?;
                }
            }
            (Kind::Output { .. }, 0) if v >= 3 => {
                r.end()?;
                self.delete(id)?;
            }
            (Kind::Compositor, 0) => {
                let new = r.u()?;
                r.end()?;
                if self.surface.id != 0 {
                    return Err(Fault(1, 2, b"surface capacity exhausted"));
                }
                self.add(new, Kind::Surface, v)?;
                self.surface.id = new;
                self.surface_generation = self.surface_generation.wrapping_add(1);
            }
            (Kind::Compositor, 1) => {
                let new = r.u()?;
                r.end()?;
                let index = (0..self.regions.len()).find(|&i| !self.objects.iter()
                    .any(|o| matches!(o.kind, Kind::Region(n) if n == i)))
                    .ok_or(Fault(1, 2, b"region table full"))?;
                self.regions[index] = Region::EMPTY;
                self.add(new, Kind::Region(index), 1)?;
            }
            (Kind::Region(_), 0) => {
                r.end()?;
                self.delete(id)?;
            }
            (Kind::Region(index), 1 | 2) => {
                let rect = [r.i()?, r.i()?, r.i()?, r.i()?];
                r.end()?;
                if rect[2] > 0 && rect[3] > 0 {
                    let region = &mut self.regions[index];
                    let slot = region.ops.get_mut(region.len).ok_or(Fault(1, 2, b"region full"))?;
                    *slot = (op == 1, rect);
                    region.len += 1;
                }
            }
            (Kind::Shm, 0) => {
                let (new, size) = (r.u()?, r.i()?);
                r.end()?;
                let invalid = Fault(id, 2, b"invalid shared memory pool");
                if size <= 0 {
                    return Err(invalid);
                }
                let map = platform::map_readonly(fd.ok_or(invalid)?).map_err(|_| invalid)?;
                if size as usize > map.len() {
                    return Err(invalid);
                }
                let slot = self
                    .pools
                    .iter()
                    .position(|p| p.map.is_none())
                    .ok_or(Fault(1, 2, b"pool table full"))?;
                self.add(new, Kind::Pool(slot), 1)?;
                self.pools[slot] = Pool {
                    map: Some(map),
                    size: size as usize,
                };
            }
            (Kind::Pool(pool), 0) => {
                let (new, offset, w, h, stride, format) =
                    (r.u()?, r.i()?, r.i()?, r.i()?, r.i()?, r.u()?);
                r.end()?;
                if format > 1 {
                    return Err(Fault(id, 0, b"unsupported SHM format"));
                }
                let bad = Fault(id, 1, b"invalid shared buffer extent");
                if offset < 0 || w <= 0 || h <= 0 || stride <= 0 {
                    return Err(bad);
                }
                let (width, height, stride, offset) =
                    (w as usize, h as usize, stride as usize, offset as usize);
                let bytes = width.checked_mul(4).ok_or(bad)?;
                let end = (height - 1)
                    .checked_mul(stride)
                    .and_then(|v| v.checked_add(offset))
                    .and_then(|v| v.checked_add(bytes))
                    .ok_or(bad)?;
                if stride < bytes
                    || width.checked_mul(height).ok_or(bad)? > PIXELS
                    || end > self.pools[pool].size
                {
                    return Err(bad);
                }
                self.add(
                    new,
                    Kind::Buffer(Buffer {
                        pool,
                        offset,
                        width,
                        height,
                        stride,
                        argb: format == 0,
                    }),
                    1,
                )?;
            }
            (Kind::Pool(_), 1) => {
                r.end()?;
                self.delete(id)?;
                self.collect();
            }
            (Kind::Pool(i), 2) => {
                let n = r.i()?;
                r.end()?;
                let p = &mut self.pools[i];
                if n <= 0 || n as usize <= p.size || n as usize > p.map.as_ref().ok_or(())?.len() {
                    return Err(Fault(id, 2, b"invalid pool resize"));
                }
                p.size = n as usize;
            }
            (Kind::Buffer(_), 0) => {
                r.end()?;
                if let Some(Some(ref mut pending)) = self.surface.pending {
                    if pending.id == id {
                        pending.id = 0;
                    }
                }
                self.delete(id)?;
                self.collect();
            }
            (Kind::Surface, 0) => {
                r.end()?;
                if self.surface.xdg != 0 {
                    return Err(Fault(
                        self.surface.xdg,
                        6,
                        b"destroy xdg_surface before wl_surface",
                    ));
                }
                self.delete(id)?;
                self.pointer_local = None;
                self.surface = Surface::new();
                self.input_region = None;
                self.pending_region = None;
                for o in self.objects {
                    if matches!(o.kind, Kind::Frame | Kind::Presented) {
                        self.delete(o.id)?;
                    }
                }
                self.collect();
                self.changed = true;
            }
            (Kind::Surface, 1) => {
                let (b, x, y) = (r.u()?, r.i()?, r.i()?);
                r.end()?;
                self.surface.pending = Some(if b == 0 {
                    None
                } else {
                    let Kind::Buffer(buffer) = self.objects[self.index(b)?].kind else {
                        return Err(Fault(1, 0, b"expected wl_buffer"));
                    };
                    Some(Attachment { id: b, buffer })
                });
                self.surface.dx = x;
                self.surface.dy = y;
                self.collect();
            }
            (Kind::Surface, 2 | 9) if op == 2 || v >= 4 => {
                for _ in 0..4 {
                    r.i()?;
                }
                r.end()?;
                self.surface.damaged = true;
            }
            (Kind::Surface, 3) => {
                let new = r.u()?;
                r.end()?;
                self.add(new, Kind::Frame, 1)?;
            }
            (Kind::Surface, 4 | 5) => {
                let reg = r.u()?;
                r.end()?;
                let region = if reg == 0 { None } else {
                    let Kind::Region(index) = self.objects[self.index(reg)?].kind else {
                        return Err(Fault(1, 0, b"expected wl_region"));
                    };
                    Some(self.regions[index])
                };
                if op == 5 { self.pending_region = Some(region); }
            }
            (Kind::Surface, 6) => {
                r.end()?;
                self.commit(last, image)?;
                if let Some(region) = self.pending_region.take() { self.input_region = region; }
            }
            (Kind::Surface, 7) if v >= 2 => {
                let t = r.i()?;
                r.end()?;
                if !(0..8).contains(&t) {
                    return Err(Fault(id, 1, b"invalid buffer transform"));
                }
                self.surface.next_transform = t as u32;
            }
            (Kind::Surface, 8) if v >= 3 => {
                let s = r.i()?;
                r.end()?;
                if s <= 0 {
                    return Err(Fault(id, 0, b"invalid buffer scale"));
                }
                self.surface.next_scale = s as usize;
            }
            (Kind::Wm, 0) => {
                r.end()?;
                if self.surface.wm == id && self.surface.xdg != 0 {
                    return Err(Fault(id, 1, b"xdg surfaces still live"));
                }
                self.delete(id)?;
            }
            (Kind::Wm, 1) => {
                let new = r.u()?;
                r.end()?;
                self.add(new, Kind::Positioner, 1)?;
            }
            (Kind::Wm, 2) => {
                let (new, surface) = (r.u()?, r.u()?);
                r.end()?;
                if surface != self.surface.id || surface == 0 {
                    return Err(Fault(1, 0, b"expected wl_surface"));
                }
                if self.surface.role_assigned
                    || self.surface.xdg != 0
                    || self.surface.width != 0
                    || self.surface.pending.flatten().is_some()
                {
                    return Err(Fault(id, 0, b"surface already has role or buffer"));
                }
                self.add(new, Kind::Xdg, 1)?;
                self.surface.xdg = new;
                self.surface.wm = id;
            }
            (Kind::Wm, 3) => {
                r.u()?;
                r.end()?;
            }
            (Kind::Positioner, 0) => {
                r.end()?;
                self.delete(id)?;
            }
            (Kind::Positioner, 1 | 2 | 3 | 4 | 5 | 6) => {
                let n = match op {
                    1 | 6 => 2,
                    2 => 4,
                    _ => 1,
                };
                for _ in 0..n {
                    r.i()?;
                }
                r.end()?;
            }
            (Kind::Xdg, 0) => {
                r.end()?;
                if self.surface.top != 0 {
                    return Err(Fault(id, 6, b"toplevel still live"));
                }
                self.delete(id)?;
                self.surface.xdg = 0;
                self.surface.wm = 0;
            }
            (Kind::Xdg, 1) => {
                let new = r.u()?;
                r.end()?;
                if self.surface.top != 0 {
                    return Err(Fault(id, 2, b"toplevel already constructed"));
                }
                self.add(new, Kind::Top, 1)?;
                self.surface.top = new;
                self.surface.role_assigned = true;
            }
            (Kind::Xdg, 3) => {
                let (x, y) = (r.i()?, r.i()?);
                let (w, h) = (r.i()?, r.i()?);
                r.end()?;
                if w <= 0 || h <= 0 {
                    return Err(Fault(id, 5, b"invalid window geometry"));
                }
                self.surface.next_geometry = Some(Geometry { x, y, width: w, height: h });
            }
            (Kind::Xdg, 4) => {
                let seq = r.u()?;
                r.end()?;
                let k = self.configures[..self.configs]
                    .iter()
                    .position(|&s| s == seq)
                    .ok_or(Fault(id, 4, b"unknown configure serial"))?
                    + 1;
                self.configures.copy_within(k..self.configs, 0);
                self.configs -= k;
                self.surface.acked = true;
            }
            (Kind::Top, 0) => {
                r.end()?;
                self.delete(id)?;
                self.surface.geometry = None;
                self.surface.next_geometry = None;
                self.surface.top = 0;
                self.surface.mapped = false;
                self.surface.configure = 0;
                self.surface.acked = false;
                self.configs = 0;
                self.changed = true;
            }
            (Kind::Top, 1) => {
                let parent = r.u()?;
                r.end()?;
                if parent != 0 {
                    return Err(Fault(id, 1, b"invalid toplevel parent"));
                }
            }
            (Kind::Top, 2 | 3) => {
                let txt = r.string()?;
                r.end()?;
                if op == 3 {
                    let n = txt.len().min(self.surface.app_id.len());
                    self.surface.app_id[..n].copy_from_slice(&txt.as_bytes()[..n]);
                    self.surface.app_len = n;
                }
            }
            (Kind::Top, 4 | 5 | 6) => {
                let seat = r.u()?;
                if !matches!(self.objects[self.index(seat)?].kind, Kind::Seat) {
                    return Err(().into());
                }
                r.u()?;
                if op == 4 {
                    r.i()?;
                    r.i()?;
                }
                if op == 6 {
                    r.u()?;
                }
                r.end()?;
            }
            (Kind::Top, 7 | 8) => {
                let (w, h) = (r.i()?, r.i()?);
                r.end()?;
                if w < 0 || h < 0 {
                    return Err(Fault(id, 2, b"invalid constraint"));
                }
            }
            (Kind::Top, 9 | 10 | 11 | 12 | 13) => {
                if op == 11 {
                    let output = r.u()?;
                    if output != 0 && !matches!(self.objects[self.index(output)?].kind, Kind::Output { .. }) {
                        return Err(Fault(1, 0, b"expected wl_output"));
                    }
                }
                r.end()?;
                if op != 13 && self.surface.configure != 0 {
                    self.configure(last)?;
                }
            }
            (Kind::Seat, 1) => {
                let new = r.u()?;
                r.end()?;
                self.add(new, Kind::Keyboard, v)?;
                input::keymap(&mut self.out, new, v)?;
                if focused && self.surface.mapped {
                    input::enter(&mut self.out, new, serial(last), self.surface.id)?;
                }
            }
            (Kind::Seat, 0) if self.pointer_available => {
                let new = r.u()?;
                r.end()?;
                self.add(new, Kind::Pointer, v)?;
                if let Some((surface, x, y)) = self.pointer_local.filter(|p| p.0 == self.surface.id && self.surface.mapped) {
                    self.out.focus_words(new, 0, &[serial(last), surface, x as u32, y as u32])?;
                }
            }
            (Kind::Pointer, 0) => {
                let (_serial, surface, _x, _y) = (r.u()?, r.u()?, r.i()?, r.i()?);
                r.end()?;
                if surface != 0 {
                    self.index(surface)?;
                    return Err(Fault(id, 0, b"surface already has a role"));
                }
            }
            (Kind::Pointer, 1) if v >= 3 => {
                r.end()?;
                self.delete(id)?;
            }
            (Kind::Seat, 0 | 2) => return Err(Fault(id, 0, b"seat capability unavailable")),
            (Kind::Keyboard, 0) if v >= 3 => {
                r.end()?;
                self.delete(id)?;
            }
            _ => return Err(Fault(1, 1, b"unsupported request or object version")),
        }
        Ok(())
    }
}
const GLOBALS: [(u32, &[u8], u32); 5] = [
    (1, b"wl_compositor", 4),
    (2, b"wl_shm", 1),
    (3, b"xdg_wm_base", 1),
    (4, b"wl_seat", 4),
    (5, b"wl_output", 3),
];
