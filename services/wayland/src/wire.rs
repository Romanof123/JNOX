use crate::platform;

pub const MAX_MESSAGE: usize = 4096;
pub struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], ()> {
        let (head, tail) = self.0.split_at_checked(n).ok_or(())?;
        self.0 = tail;
        Ok(head)
    }
    pub fn u(&mut self) -> Result<u32, ()> {
        Ok(u32::from_ne_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i(&mut self) -> Result<i32, ()> {
        Ok(self.u()? as i32)
    }
    pub fn string(&mut self) -> Result<&'a str, ()> {
        let n = self.u()? as usize;
        if n == 0 || n > MAX_MESSAGE {
            return Err(());
        }
        let field = &self.take(n.next_multiple_of(4))?[..n];
        if field[n - 1] != 0 || field[..n - 1].contains(&0) {
            return Err(());
        }
        core::str::from_utf8(&field[..n - 1]).map_err(|_| ())
    }
    pub fn end(&self) -> Result<(), ()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub struct In {
    bytes: [u8; MAX_MESSAGE],
    len: usize,
    fds: [i32; 8],
    count: usize,
}
impl In {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_MESSAGE],
            len: 0,
            fds: [-1; 8],
            count: 0,
        }
    }
    pub fn receive(&mut self, fd: i32) -> Result<usize, i32> {
        if self.len == MAX_MESSAGE {
            return Err(-90);
        }
        let mut rights = [-1; 8];
        let (bytes, count) = platform::recv(fd, &mut self.bytes[self.len..], &mut rights)?;
        if bytes != 0 {
            crate::observe::wire(
                fd,
                1,
                &self.bytes[self.len..self.len + bytes],
                &rights[..count],
            );
        }
        if count > self.fds.len() - self.count {
            for &right in &rights[..count] {
                platform::close(right);
            }
            return Err(-24);
        }
        self.fds[self.count..self.count + count].copy_from_slice(&rights[..count]);
        self.count += count;
        self.len += bytes;
        Ok(bytes)
    }
    pub fn header(&self) -> Result<Option<(u32, u16, usize)>, ()> {
        if self.len < 8 {
            return Ok(None);
        }
        let mut r = Reader::new(&self.bytes[..8]);
        let obj = r.u()?;
        let word = r.u()?;
        let size = (word >> 16) as usize;
        if obj == 0 || !(8..=MAX_MESSAGE).contains(&size) || size % 4 != 0 {
            return Err(());
        }
        Ok((self.len >= size).then_some((obj, word as u16, size)))
    }
    pub fn pop_fd(&mut self) -> Option<i32> {
        if self.count == 0 {
            return None;
        }
        let fd = self.fds[0];
        self.fds.copy_within(1..self.count, 0);
        self.count -= 1;
        Some(fd)
    }
    pub fn consume(&mut self, size: usize, dst: &mut [u8]) {
        dst.copy_from_slice(&self.bytes[8..size]);
        self.bytes.copy_within(size..self.len, 0);
        self.len -= size;
    }
    pub fn clear(&mut self) {
        while let Some(fd) = self.pop_fd() {
            platform::close(fd);
        }
        self.len = 0;
    }
}

#[derive(Copy, Clone)]
struct Marker {
    offset: usize,
    fd: i32,
}
pub struct Out {
    bytes: [u8; 8192],
    len: usize,
    markers: [Marker; 8],
    count: usize,
}
impl Out {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 8192],
            len: 0,
            markers: [Marker { offset: 0, fd: -1 }; 8],
            count: 0,
        }
    }
    pub fn event(&mut self, id: u32, op: u16, body: &[u8]) -> Result<(), ()> {
        let size = body.len() + 8;
        if size % 4 != 0 || size > self.bytes.len() - self.len {
            return Err(());
        }
        let tail = &mut self.bytes[self.len..self.len + size];
        tail[..4].copy_from_slice(&id.to_ne_bytes());
        tail[4..8].copy_from_slice(&(((size as u32) << 16) | u32::from(op)).to_ne_bytes());
        tail[8..].copy_from_slice(body);
        self.len += size;
        Ok(())
    }
    pub fn event_fd(&mut self, id: u32, op: u16, body: &[u8], fd: i32) -> Result<(), ()> {
        let offset = self.len;
        if self.count == self.markers.len() || self.event(id, op, body).is_err() {
            platform::close(fd);
            return Err(());
        }
        self.markers[self.count] = Marker { offset, fd };
        self.count += 1;
        Ok(())
    }
    pub fn words(&mut self, id: u32, op: u16, words: &[u32]) -> Result<(), ()> {
        let mut body = [0; 32];
        if words.len() > body.len() / 4 {
            return Err(());
        }
        for (dst, word) in body.chunks_exact_mut(4).zip(words) {
            dst.copy_from_slice(&word.to_ne_bytes());
        }
        self.event(id, op, &body[..4 * words.len()])
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn flush(&mut self, sock: i32) -> Result<bool, i32> {
        if self.len == 0 {
            return Ok(false);
        }
        let attached = self.count != 0 && self.markers[0].offset == 0;
        let next = usize::from(attached);
        let n = if next < self.count {
            self.markers[next].offset
        } else {
            self.len
        };
        let fd = attached.then(|| self.markers[0].fd);
        let sent = platform::send(sock, &self.bytes[..n], fd)?;
        if sent == 0 || sent > n {
            return Err(-32);
        }
        crate::observe::wire(sock, 2, &self.bytes[..sent], fd.as_slice());
        if let Some(fd) = fd {
            platform::close(fd);
            self.markers.copy_within(1..self.count, 0);
            self.count -= 1;
        }
        for m in &mut self.markers[..self.count] {
            m.offset -= sent;
        }
        self.bytes.copy_within(sent..self.len, 0);
        self.len -= sent;
        Ok(true)
    }
    pub fn clear(&mut self) {
        for m in &self.markers[..self.count] {
            platform::close(m.fd);
        }
        self.count = 0;
        self.len = 0;
    }
}

pub fn with_string(dst: &mut [u8], text: &[u8]) -> usize {
    let n = text.len() + 1;
    let end = 4 + n.next_multiple_of(4);
    dst[..end].fill(0);
    dst[..4].copy_from_slice(&(n as u32).to_ne_bytes());
    dst[4..4 + text.len()].copy_from_slice(text);
    end
}
