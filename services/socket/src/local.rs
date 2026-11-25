//! Bounded AF_UNIX engine for the EL0 broker. The broker supplies the authenticated
//! caller slot. Every operation is nonblocking; only -EAGAIN is eligible for retry.
const CLIENTS: usize = 3;
const OBJECTS: usize = 8;
const FDS: usize = 4;
const STREAM: u8 = 1;
const DGRAM: u8 = 2;
const SEQ: u8 = 5;

#[derive(Clone, Copy)]
struct Name {
    bytes: [u8; 31],
    len: usize,
}
impl Name {
    const EMPTY: Self = Self {
        bytes: [0; 31],
        len: 0,
    };
    fn parse(path: &[u8]) -> Result<Self, i32> {
        if path.len() > 31 {
            return Err(-36);
        }
        if path.is_empty() || path.contains(&0) {
            return Err(-22);
        }
        let mut n = Self::EMPTY;
        n.bytes[..path.len()].copy_from_slice(path);
        n.len = path.len();
        Ok(n)
    }
    fn matches(&self, other: Self) -> bool {
        self.bytes[..self.len] == other.bytes[..other.len]
    }
}

#[derive(Clone, Copy)]
struct Record {
    data: [u8; 32],
    len: usize,
    source: Name,
}
impl Record {
    const EMPTY: Self = Self {
        data: [0; 32],
        len: 0,
        source: Name::EMPTY,
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Link {
    Unconnected,
    Peer(usize),
    Closed,
}
#[derive(Clone, Copy)]
struct Socket {
    kind: u8,
    name: Name,
    bound: bool,
    peer: Link,
    read_open: bool,
    write_open: bool,
    backlog: Option<usize>,
    pending: [usize; 2],
    npending: usize,
    bytes: [u8; 64],
    byte_head: usize,
    nbytes: usize,
    records: [Record; 4],
    rec_head: usize,
    nrecords: usize,
}
impl Socket {
    const EMPTY: Self = Self {
        kind: 0,
        name: Name::EMPTY,
        bound: false,
        peer: Link::Unconnected,
        read_open: true,
        write_open: true,
        backlog: None,
        pending: [0; 2],
        npending: 0,
        bytes: [0; 64],
        byte_head: 0,
        nbytes: 0,
        records: [Record::EMPTY; 4],
        rec_head: 0,
        nrecords: 0,
    };
}

#[derive(Debug, PartialEq, Eq)]
pub struct Received {
    pub count: usize,
    pub full_len: usize,
    pub source: [u8; 31],
    pub source_len: usize,
}
impl Received {
    fn new(count: usize, full_len: usize, source: Name) -> Self {
        Self {
            count,
            full_len,
            source: source.bytes,
            source_len: source.len,
        }
    }
}

pub struct Local {
    objects: [Socket; OBJECTS],
    fds: [[Option<usize>; FDS]; CLIENTS],
}
impl Local {
    pub const fn new() -> Self {
        Self {
            objects: [Socket::EMPTY; OBJECTS],
            fds: [[None; FDS]; CLIENTS],
        }
    }
    fn id(&self, owner: usize, fd: u8) -> Result<usize, i32> {
        self.fds
            .get(owner)
            .and_then(|fds| fds.get(fd as usize))
            .copied()
            .flatten()
            .ok_or(-9)
    }
    fn free_fd(&self, owner: usize) -> Result<usize, i32> {
        self.fds
            .get(owner)
            .ok_or(-13)?
            .iter()
            .position(Option::is_none)
            .ok_or(-24)
    }
    fn free_object(&self) -> Result<usize, i32> {
        self.objects.iter().position(|s| s.kind == 0).ok_or(-23)
    }
    fn lookup(&self, name: Name) -> Result<usize, i32> {
        self.objects
            .iter()
            .position(|s| s.bound && s.name.matches(name))
            .ok_or(-2)
    }

    pub fn socket(&mut self, owner: usize, kind: u8) -> Result<u8, i32> {
        if !matches!(kind, STREAM | DGRAM | SEQ) {
            return Err(-94);
        }
        let fd = self.free_fd(owner)?;
        let id = self.free_object()?;
        self.objects[id] = Socket {
            kind,
            ..Socket::EMPTY
        };
        self.fds[owner][fd] = Some(id);
        Ok(fd as u8)
    }
    pub fn bind(&mut self, owner: usize, fd: u8, path: &[u8]) -> Result<(), i32> {
        let id = self.id(owner, fd)?;
        let name = Name::parse(path)?;
        if self.objects[id].bound || self.objects[id].peer != Link::Unconnected {
            return Err(-22);
        }
        if self.lookup(name).is_ok() {
            return Err(-98);
        }
        self.objects[id].name = name;
        self.objects[id].bound = true;
        Ok(())
    }
    pub fn listen(&mut self, owner: usize, fd: u8, backlog: usize) -> Result<(), i32> {
        let id = self.id(owner, fd)?;
        let s = &mut self.objects[id];
        if s.kind == DGRAM {
            return Err(-95);
        }
        if !s.bound || s.peer != Link::Unconnected || !(1..=2).contains(&backlog) {
            return Err(-22);
        }
        s.backlog = Some(backlog);
        Ok(())
    }
    pub fn connect(&mut self, owner: usize, fd: u8, path: &[u8]) -> Result<(), i32> {
        let id = self.id(owner, fd)?;
        let remote = self.lookup(Name::parse(path)?)?;
        let kind = self.objects[id].kind;
        if self.objects[remote].kind != kind {
            return Err(-91);
        }
        if kind == DGRAM {
            self.objects[id].peer = Link::Peer(remote);
            return Ok(());
        }
        if self.objects[id].peer != Link::Unconnected || self.objects[id].backlog.is_some() {
            return Err(-106);
        }
        let max = self.objects[remote].backlog.ok_or(-111)?;
        if self.objects[remote].npending >= max {
            return Err(-11);
        }
        let child = self.free_object()?;
        // Reserve the accepted endpoint now: writes before accept use its private queue.
        self.objects[child] = Socket {
            kind,
            name: self.objects[remote].name,
            peer: Link::Peer(id),
            ..Socket::EMPTY
        };
        self.objects[id].peer = Link::Peer(child);
        let listener = &mut self.objects[remote];
        listener.pending[listener.npending] = child;
        listener.npending += 1;
        Ok(())
    }
    pub fn accept(&mut self, owner: usize, fd: u8) -> Result<u8, i32> {
        let id = self.id(owner, fd)?;
        let free = self.free_fd(owner)?;
        let l = &mut self.objects[id];
        if l.backlog.is_none() {
            return Err(-22);
        }
        if l.npending == 0 {
            return Err(-11);
        }
        self.fds[owner][free] = Some(l.pending[0]);
        l.pending.copy_within(1..l.npending, 0);
        l.npending -= 1;
        Ok(free as u8)
    }
    pub fn send(
        &mut self,
        owner: usize,
        fd: u8,
        data: &[u8],
        to: Option<&[u8]>,
    ) -> Result<usize, i32> {
        let id = self.id(owner, fd)?;
        let src = &self.objects[id];
        if !src.write_open {
            return Err(-32);
        }
        if src.kind != STREAM && data.len() > 32 {
            return Err(-90);
        }
        let dst = match to {
            Some(path) if src.kind == DGRAM => self.lookup(Name::parse(path)?)?,
            Some(_) => return Err(-106),
            None => match src.peer {
                Link::Peer(n) => n,
                Link::Closed => return Err(-32),
                Link::Unconnected => return Err(if src.kind == DGRAM { -89 } else { -107 }),
            },
        };
        let name = src.name;
        let kind = src.kind;
        let d = &mut self.objects[dst];
        if kind != d.kind {
            return Err(-91);
        }
        if !d.read_open {
            return Err(-32);
        }
        if kind == DGRAM && !matches!(d.peer, Link::Unconnected) && d.peer != Link::Peer(id) {
            return Err(-1);
        }
        if kind == STREAM {
            let n = data.len().min(d.bytes.len() - d.nbytes);
            if n == 0 && !data.is_empty() {
                return Err(-11);
            }
            for &b in &data[..n] {
                d.bytes[(d.byte_head + d.nbytes) % 64] = b;
                d.nbytes += 1;
            }
            Ok(n)
        } else {
            if d.nrecords == 4 {
                return Err(-11);
            }
            let mut msg = Record {
                len: data.len(),
                source: name,
                ..Record::EMPTY
            };
            msg.data[..data.len()].copy_from_slice(data);
            d.records[(d.rec_head + d.nrecords) % 4] = msg;
            d.nrecords += 1;
            Ok(data.len())
        }
    }
    pub fn recv(&mut self, owner: usize, fd: u8, out: &mut [u8]) -> Result<Received, i32> {
        let id = self.id(owner, fd)?;
        let s = &self.objects[id];
        if s.kind != DGRAM && s.peer == Link::Unconnected {
            return Err(-107);
        }
        let eof = s.kind != DGRAM
            && match s.peer {
                Link::Closed => true,
                Link::Peer(n) => !self.objects[n].write_open,
                Link::Unconnected => false,
            };
        let s = &mut self.objects[id];
        if !s.read_open {
            return Ok(Received::new(0, 0, Name::EMPTY));
        }
        if s.kind == STREAM {
            if s.nbytes == 0 && !out.is_empty() && !eof {
                return Err(-11);
            }
            let n = out.len().min(s.nbytes);
            for b in &mut out[..n] {
                *b = s.bytes[s.byte_head];
                s.byte_head = (s.byte_head + 1) % 64;
            }
            s.nbytes -= n;
            Ok(Received::new(n, n, Name::EMPTY))
        } else {
            if s.nrecords == 0 && s.kind == DGRAM && s.peer == Link::Closed {
                return Err(-111);
            }
            if s.nrecords == 0 {
                return if eof {
                    Ok(Received::new(0, 0, Name::EMPTY))
                } else {
                    Err(-11)
                };
            }
            let msg = s.records[s.rec_head];
            s.records[s.rec_head] = Record::EMPTY;
            s.rec_head = (s.rec_head + 1) % 4;
            s.nrecords -= 1;
            let n = out.len().min(msg.len);
            out[..n].copy_from_slice(&msg.data[..n]);
            Ok(Received::new(n, msg.len, msg.source))
        }
    }
    pub fn shutdown(&mut self, owner: usize, fd: u8, how: u8) -> Result<(), i32> {
        let id = self.id(owner, fd)?;
        let s = &mut self.objects[id];
        if how > 2 {
            return Err(-22);
        }
        if s.peer == Link::Unconnected {
            return Err(-107);
        }
        if how != 1 {
            s.read_open = false;
            s.nbytes = 0;
            s.nrecords = 0;
        }
        if how != 0 {
            s.write_open = false;
        }
        Ok(())
    }
    fn discard(&mut self, id: usize) {
        for s in &mut self.objects {
            if s.peer == Link::Peer(id) {
                s.peer = Link::Closed;
            }
        }
        self.objects[id] = Socket::EMPTY;
    }
    pub fn close(&mut self, owner: usize, fd: u8) -> Result<(), i32> {
        let id = self.id(owner, fd)?;
        self.fds[owner][fd as usize] = None;
        let pending = self.objects[id].pending;
        let n = self.objects[id].npending;
        for &child in &pending[..n] {
            self.discard(child);
        }
        self.discard(id);
        Ok(())
    }
    pub fn exit(&mut self, owner: usize) {
        if owner < CLIENTS {
            for fd in 0..FDS {
                if self.fds[owner][fd].is_some() {
                    let _ = self.close(owner, fd as u8);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pair(s: &mut Local, k: u8) -> (u8, u8, u8) {
        let l = s.socket(0, k).unwrap();
        s.bind(0, l, b"/service").unwrap();
        s.listen(0, l, 2).unwrap();
        let c = s.socket(1, k).unwrap();
        s.connect(1, c, b"/service").unwrap();
        (l, c, s.accept(0, l).unwrap())
    }
    #[test]
    fn stream_coalesces_splits_partially_writes_and_drains_before_eof() {
        let mut s = Local::new();
        let (_, c, a) = pair(&mut s, STREAM);
        assert_eq!(s.send(2, c, b"x", None), Err(-9));
        assert_eq!(s.send(1, c, b"ab", None), Ok(2));
        assert_eq!(s.send(1, c, &[7; 64], None), Ok(62));
        assert_eq!(s.send(1, c, b"z", None), Err(-11));
        let mut out = [0; 3];
        assert_eq!(s.recv(0, a, &mut out).unwrap().count, 3);
        assert_eq!(out, [b'a', b'b', 7]);
        assert_eq!(s.send(1, c, b"xyz", None), Ok(3));
        s.shutdown(1, c, 1).unwrap();
        assert_eq!(s.send(1, c, b"a", None), Err(-32));
        let mut rest = [0; 64];
        assert_eq!(s.recv(0, a, &mut rest).unwrap().count, 64);
        assert_eq!(&rest[..61], &[7; 61]);
        assert_eq!(&rest[61..], b"xyz");
        assert_eq!(s.recv(0, a, &mut rest).unwrap().count, 0);
        s.send(0, a, b"reply", None).unwrap();
        assert_eq!(s.recv(1, c, &mut rest).unwrap().count, 5);
        s.exit(1);
        assert_eq!(s.send(0, a, b"x", None), Err(-32));
    }

    #[test]
    fn datagrams_preserve_source_truncation_and_old_peer_identity() {
        let mut s = Local::new();
        let a = s.socket(0, DGRAM).unwrap();
        let b = s.socket(1, DGRAM).unwrap();
        s.bind(0, a, b"/a").unwrap();
        s.bind(1, b, b"/b").unwrap();
        s.connect(0, a, b"/b").unwrap();
        s.send(1, b, b"abc", Some(b"/a")).unwrap();
        let mut out = [0; 2];
        let r = s.recv(0, a, &mut out).unwrap();
        assert_eq!((r.count, r.full_len), (2, 3));
        assert_eq!(&r.source[..r.source_len], b"/b");
        assert_eq!(&out, b"ab");
        assert_eq!(s.recv(0, a, &mut out), Err(-11));
        let stranger = s.socket(2, DGRAM).unwrap();
        assert_eq!(s.send(2, stranger, b"x", Some(b"/a")), Err(-1));
        s.exit(1);
        let new = s.socket(2, DGRAM).unwrap();
        s.bind(2, new, b"/b").unwrap();
        assert_eq!(s.send(0, a, b"old", None), Err(-32));
        assert_eq!(s.recv(0, a, &mut out), Err(-111));
        s.connect(0, a, b"/b").unwrap();
        s.send(0, a, b"x", None).unwrap();
        assert_eq!(s.recv(2, new, &mut out).unwrap().count, 1);
    }

    #[test]
    fn seqpacket_capacity_order_and_drain_after_peer_exit() {
        let mut s = Local::new();
        let (_, c, a) = pair(&mut s, SEQ);
        assert_eq!(s.send(1, c, &[0; 33], None), Err(-90));
        for i in 0..4 {
            s.send(1, c, &[i; 8], None).unwrap();
        }
        assert_eq!(s.send(1, c, b"extra", None), Err(-11));
        s.exit(1);
        for i in 0..4 {
            let mut b = [0; 5];
            let r = s.recv(0, a, &mut b).unwrap();
            assert_eq!((r.count, r.full_len, b), (5, 8, [i; 5]));
        }
        assert_eq!(s.recv(0, a, &mut [0; 1]).unwrap().count, 0);
    }

    #[test]
    fn backlog_object_and_owner_limits_reclaim_on_exit() {
        let mut s = Local::new();
        let l = s.socket(0, STREAM).unwrap();
        assert_eq!(s.bind(0, l, &[1; 32]), Err(-36));
        s.bind(0, l, b"/s").unwrap();
        s.listen(0, l, 2).unwrap();
        let a = s.socket(1, STREAM).unwrap();
        let b = s.socket(1, STREAM).unwrap();
        let c = s.socket(2, STREAM).unwrap();
        for fd in [a, b] {
            s.connect(1, fd, b"/s").unwrap();
        }
        assert_eq!(s.connect(2, c, b"/s"), Err(-11));
        let accepted = s.accept(0, l).unwrap();
        s.connect(2, c, b"/s").unwrap();
        let last = s.socket(1, STREAM).unwrap();
        assert_eq!(s.socket(2, STREAM), Err(-23));
        s.close(1, last).unwrap();
        s.close(0, l).unwrap();
        assert_eq!(s.send(1, b, b"x", None), Err(-32));
        assert_eq!(s.send(2, c, b"x", None), Err(-32));
        assert_eq!(s.send(1, a, b"x", None), Ok(1));
        s.close(0, accepted).unwrap();
        s.exit(0);
        s.exit(1);
        s.exit(2);
        for _ in 0..4 {
            s.socket(1, DGRAM).unwrap();
        }
        assert_eq!(s.socket(1, DGRAM), Err(-24));
    }
}
