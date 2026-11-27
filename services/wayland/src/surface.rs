use crate::platform::Mapping;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;
pub const PIXELS: usize = WIDTH * HEIGHT;
#[derive(Clone, Copy)]
pub struct Buffer {
    pub pool: usize,
    pub offset: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub argb: bool,
}
#[derive(Clone, Copy)]
pub struct Attachment {
    pub id: u32,
    pub buffer: Buffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct Surface {
    pub next_geometry: Option<Geometry>,
    pub geometry: Option<Geometry>,
    pub id: u32,
    pub wm: u32,
    pub xdg: u32,
    pub top: u32,
    pub role_assigned: bool,
    pub configure: u32,
    pub acked: bool,
    pub pending: Option<Option<Attachment>>,
    pub scale: usize,
    pub transform: u32,
    pub next_scale: usize,
    pub next_transform: u32,
    pub width: usize,
    pub height: usize,
    pub mapped: bool,
    pub damaged: bool,
    pub app_id: [u8; 64],
    pub app_len: usize,
    pub dx: i32,
    pub dy: i32,
    pub content_x: i32,
    pub content_y: i32,
}
impl Surface {
    pub const fn new() -> Self {
        Self {
            next_geometry: None,
            geometry: None,
            id: 0,
            wm: 0,
            xdg: 0,
            top: 0,
            role_assigned: false,
            configure: 0,
            acked: false,
            pending: None,
            scale: 1,
            transform: 0,
            next_scale: 1,
            next_transform: 0,
            width: 0,
            height: 0,
            mapped: false,
            damaged: false,
            app_id: [0; 64],
            app_len: 0,
            dx: 0,
            dy: 0,
            content_x: 0,
            content_y: 0,
        }
    }
    pub fn commit_geometry(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let Some(rect) = self.next_geometry.take() else {
            return;
        };
        let (w, h) = if self.transform & 1 == 0 {
            (self.width, self.height)
        } else {
            (self.height, self.width)
        };
        // Geometry is surface-local. Clamp once, against this commit's
        // logical bounds, including its buffer transform and scale.
        let axis = |start: i32, size: i32, bound: usize| {
            let bound = (bound / self.scale) as i64;
            let first = i64::from(start).clamp(0, bound);
            let end = (i64::from(start) + i64::from(size)).clamp(0, bound);
            (first as i32, (end - first) as i32)
        };
        let (x, width) = axis(rect.x, rect.width, w);
        let (y, height) = axis(rect.y, rect.height, h);
        self.geometry = Some(Geometry { x, y, width, height });
    }

    pub fn buffer_origin(&self, anchor: (i64, i64)) -> (i64, i64) {
        let (gx, gy) = self.geometry.map_or((0, 0), |rect| (rect.x, rect.y));
        (
            anchor.0 + i64::from(self.content_x) - i64::from(gx),
            anchor.1 + i64::from(self.content_y) - i64::from(gy),
        )
    }

    pub fn overlaps_output(&self, anchor: (i64, i64)) -> bool {
        if !self.mapped {
            return false;
        }
        let (left, top) = self.buffer_origin(anchor);
        let (w, h) = if self.transform & 1 == 0 {
            (self.width, self.height)
        } else {
            (self.height, self.width)
        };
        let (w, h) = ((w / self.scale) as i64, (h / self.scale) as i64);
        w > 0 && h > 0 && left < WIDTH as i64 && top < HEIGHT as i64 && left + w > 0 && top + h > 0
    }

    pub fn pixel(&self, x: usize, y: usize, image: &[u32]) -> Option<u32> {
        if !self.mapped {
            return None;
        }
        let (u, v) = (x.checked_mul(self.scale)?, y.checked_mul(self.scale)?);
        let (w, h) = (self.width, self.height);
        let (sw, sh) = if self.transform & 1 == 0 {
            (w, h)
        } else {
            (h, w)
        };
        if u >= sw || v >= sh {
            return None;
        }
        // wl_output.transform is the transform that the client applied when
        // producing the buffer. Apply it to logical coordinates to sample.
        let (x, y) = match self.transform {
            0 => (u, v),
            1 => (v, h - 1 - u),
            2 => (w - 1 - u, h - 1 - v),
            3 => (w - 1 - v, u),
            4 => (w - 1 - u, v),
            5 => (w - 1 - v, h - 1 - u),
            6 => (u, h - 1 - v),
            7 => (v, u),
            _ => return None,
        };
        Some(image[y * w + x])
    }
}

// One synchronous CPU0 dispatcher owns staging. A host file may shrink
// between rows: callers only observe a new image after the entire copy.
static mut STAGING: [u32; PIXELS] = [0; PIXELS];
pub fn snapshot(buffer: Buffer, map: &Mapping, image: &mut [u32]) -> Result<(), ()> {
    if buffer.width.checked_mul(buffer.height).ok_or(())? > image.len().min(PIXELS) { return Err(()); }
    let staging = unsafe { &mut *(&raw mut STAGING) };
    for y in 0..buffer.height {
        let row = &mut staging[y * buffer.width..(y + 1) * buffer.width];
        // All u32 bit patterns are valid; this is private aligned storage.
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(row.as_mut_ptr().cast::<u8>(), row.len() * 4)
        };
        crate::platform::read_shm(map, buffer.offset + y * buffer.stride, bytes)?;
        if !buffer.argb {
            for pixel in row {
                *pixel |= 0xff00_0000;
            }
        }
    }
    let n = buffer.width * buffer.height;
    image[..n].copy_from_slice(&staging[..n]);
    Ok(())
}
pub fn over(src: u32, dst: u32) -> u32 {
    let inv = 255 - (src >> 24);
    let channel = |s: u32, d: u32| (s + (d * inv + 127) / 255).min(255);
    (channel((src >> 16) & 255, (dst >> 16) & 255) << 16)
        | (channel((src >> 8) & 255, (dst >> 8) & 255) << 8)
        | channel(src & 255, dst & 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_uses_committed_logical_bounds_and_preserves_shadow() {
        let mut s = Surface::new();
        s.width = 12;
        s.height = 16;
        s.scale = 2;
        s.transform = 1;
        s.next_geometry = Some(Geometry {
            x: 2,
            y: -3,
            width: i32::MAX,
            height: 8,
        });
        assert_eq!(s.buffer_origin((1, 5)), (1, 5));
        s.commit_geometry();
        assert_eq!(s.geometry, Some(Geometry {
            x: 2,
            y: 0,
            width: 6,
            height: 5,
        }));
        // The first logical column is offscreen and the second is shadow.
        assert_eq!(s.buffer_origin((1, 5)), (-1, 5));
        s.height = 24;
        s.commit_geometry();
        assert_eq!(s.geometry.unwrap().width, 6);
        s.next_geometry = Some(Geometry { x: 3, y: 1, width: 4, height: 2 });
        assert_eq!(s.buffer_origin((1, 5)), (-1, 5));
        s.commit_geometry();
        assert_eq!(s.buffer_origin((1, 5)), (-2, 4));
    }
}
