//! Volatile /tmp in the single-threaded EL0 service. The kernel authenticates
//! the caller and announces its exit. A successful reply commits a trial bank.
use super::CLIENTS;
use core::arch::asm;
use jnox_native::ipc;

const FILES: usize = 4;
const CAPACITY: usize = 1024;
const FDS: usize = 8;
const BASE: u8 = 32;
const PAYLOAD: usize = 64;
const DESCRIPTIONS: usize = CLIENTS * FDS;
const READ: u8 = 1;
const WRITE: u8 = 2;
const CREATE: u8 = 4;
const EXCL: u8 = 8;
const TRUNC: u8 = 16;
const APPEND: u8 = 32;
const CLOEXEC: u8 = 64;

#[derive(Clone, Copy)]
struct Stamp { seconds: i64, micros: u32, valid: bool }
impl Stamp {
    const UNSET: Self = Self { seconds: 0, micros: 0, valid: false };
    fn now() -> Self {
        let mut time = [0i64; 2];
        let rc: i64;
        unsafe { asm!("svc #0x4a01", in("x8") 207u64, inout("x0") time.as_mut_ptr() as usize => rc,
                     inout("x1") 0u64 => _, inout("x2") 0u64 => _, options(nostack)) };
        if rc != 0 || !(0..1_000_000).contains(&time[1]) { return Self::UNSET; }
        Self { seconds: time[0], micros: time[1] as u32, valid: true }
    }
}

#[derive(Clone, Copy)]
struct Node {
    ino: u32, mode: u16, size: usize, length: usize, name: [u8; 31],
    times: [Stamp; 3], data: [u8; CAPACITY],
}
impl Node {
    const EMPTY: Self = Self { ino: 0, mode: 0, size: 0, length: 0, name: [0; 31],
                              times: [Stamp::UNSET; 3], data: [0; CAPACITY] };
}

#[derive(Clone, Copy)]
struct Open { node: usize, flags: u8, position: i64 }
impl Open { const EMPTY: Self = Self { node: 0, flags: 0, position: 0 }; }
#[derive(Clone, Copy)]
struct Fs {
    files: [Node; FILES], open: [Open; DESCRIPTIONS], handles: [[u8; FDS]; CLIENTS],
    next_ino: u32, root_time: Stamp, dir_times: [Stamp; 3],
}
enum Path<'a> { Root, Tmp, Leaf(&'a [u8]), Outside }
fn path(raw: &[u8]) -> Result<Path<'_>, i32> {
    if raw == b"/" { return Ok(Path::Root); }
    if raw == b"/tmp" { return Ok(Path::Tmp); }
    if raw.is_empty() { return Err(-2); }
    if raw[0] != b'/' || raw[1..].split(|b| *b == b'/')
        .any(|s| s.is_empty() || s == b"." || s == b".." || s.contains(&0)) { return Err(-22); }
    let Some(name) = raw.strip_prefix(b"/tmp/") else { return Ok(Path::Outside) };
    if name.contains(&b'/') { return Err(-2); }
    if name.len() > 31 { return Err(-36); }
    Ok(Path::Leaf(name))
}
impl Fs {
    const EMPTY: Self = Self { files: [Node::EMPTY; FILES], open: [Open::EMPTY; DESCRIPTIONS],
        handles: [[0; FDS]; CLIENTS], next_ino: 3, root_time: Stamp::UNSET, dir_times: [Stamp::UNSET; 3] };
    fn lookup(&self, name: &[u8]) -> Option<usize> {
        self.files.iter().position(|f| f.ino != 0 && f.length != 0 && &f.name[..f.length] == name)
    }
    fn desc(&self, id: usize, fd: u8) -> Result<(usize, usize), i32> {
        let slot = usize::from(fd.checked_sub(BASE).ok_or(-9)?);
        let handle = *self.handles[id].get(slot).ok_or(-9)? & 127;
        if handle == 0 { return Err(-9); }
        Ok((slot, usize::from(handle - 1)))
    }
    fn reap(&mut self) {
        for (idx, d) in self.open.iter_mut().enumerate() {
            if d.flags != 0 && !self.handles.iter().flatten().any(|v| usize::from(v & 127) == idx + 1) {
                *d = Open::EMPTY;
            }
        }
        for (idx, f) in self.files.iter_mut().enumerate() {
            if f.ino != 0 && f.length == 0 && !self.open.iter().any(|d| d.flags != 0 && d.node == idx) {
                f.ino = 0;
            }
        }
    }
    fn opened(&mut self, id: usize, grant: u8, raw: &[u8], flags: u8, mode: u32) -> Result<i64, i32> {
        if flags & 128 != 0 || flags & 3 == 0 || mode > 0o777 ||
            flags & TRUNC != 0 && flags & WRITE == 0 { return Err(-22); }
        let name = match path(raw)? {
            Path::Leaf(n) => n, Path::Outside => return Err(if flags & (WRITE|CREATE) != 0 { -30 } else { -2 }),
            _ => return Err(-21),
        };
        if grant != 2 && flags & (WRITE | CREATE | TRUNC) != 0 { return Err(-13); }
        let slot = self.handles[id].iter().position(|h| *h == 0).ok_or(-24)?;
        let desc = self.open.iter().position(|d| d.flags == 0).ok_or(-23)?;
        let node = if let Some(i) = self.lookup(name) {
            let f = &mut self.files[i];
            if flags & (CREATE | EXCL) == (CREATE | EXCL) { return Err(-17); }
            if flags & READ != 0 && f.mode & 0o400 == 0 || flags & WRITE != 0 && f.mode & 0o200 == 0 {
                return Err(-13);
            }
            if flags & TRUNC != 0 {
                f.size = 0;
                let now = Stamp::now(); f.times[1..].fill(now);
            }
            i
        } else {
            if flags & CREATE == 0 { return Err(-2); }
            let i = self.files.iter().position(|f| f.ino == 0).ok_or(-28)?;
            let ino = self.next_ino;
            self.next_ino = ino.checked_add(1).ok_or(-75)?;
            let f = &mut self.files[i];
            f.ino = ino; f.size = 0; f.mode = mode as u16; f.length = name.len();
            f.name[..name.len()].copy_from_slice(name);
            let now = Stamp::now(); f.times = [now; 3]; self.dir_times[1..].fill(now);
            i
        };
        self.open[desc] = Open { node, flags, position: 0 };
        self.handles[id][slot] = (desc + 1) as u8 | if flags & CLOEXEC != 0 { 128 } else { 0 };
        Ok(i64::from(BASE) + slot as i64)
    }
    fn operate(&mut self, id: usize, grant: u8, word: u64, x: &mut Exchange) -> Result<i64, i32> {
        let op = (word >> 48) as u8; let fd = (word >> 40) as u8;
        let flags = (word >> 32) as u8; let arg = word as u32;
        let input = &x.data[..x.used];
        if op == 1 { return self.opened(id, grant, input, flags, arg); }
        if op == 2 {
            let name = match path(input)? { Path::Leaf(n) => n, Path::Outside => return Err(-30), _ => return Err(-21) };
            if grant != 2 { return Err(-13); }
            let i = self.lookup(name).ok_or(-2)?;
            self.files[i].length = 0;
            let now = Stamp::now(); self.files[i].times[2] = now; self.dir_times[1..].fill(now);
            self.reap(); return Ok(0);
        }
        if op == 3 {
            let info = match path(input)? {
                Path::Root => (1, 0o40555, 3, 0, [self.root_time; 3]),
                Path::Tmp => (2, 0o40777, 2, 0, self.dir_times),
                Path::Outside => return Err(-2),
                Path::Leaf(name) => self.info(self.lookup(name).ok_or(-2)?),
            };
            return x.stat(info);
        }
        let (slot, desc) = self.desc(id, fd)?;
        let d = self.open[desc];
        let f = &mut self.files[d.node];
        match op {
            4 => { self.handles[id][slot] = 0; self.reap(); Ok(0) }
            5 => x.stat(self.info(d.node)),
            6 => {
                if d.flags & READ == 0 { return Err(-9); }
                let n = (arg as usize).min(PAYLOAD).min(f.size.saturating_sub(d.position as usize));
                if n != 0 {
                    let pos = d.position as usize;
                    x.data[..n].copy_from_slice(&f.data[pos..pos+n]); f.times[0] = Stamp::now();
                }
                x.reply = n; self.open[desc].position += n as i64; Ok(n as i64)
            }
            7 => {
                if d.flags & WRITE == 0 { return Err(-9); }
                if input.is_empty() { return Ok(0); }
                let pos = if d.flags & APPEND != 0 { f.size } else { d.position as usize };
                let n = input.len().min(CAPACITY.saturating_sub(pos));
                if n == 0 { return Err(-28); }
                if pos > f.size { f.data[f.size..pos].fill(0); }
                f.data[pos..pos+n].copy_from_slice(&input[..n]); f.size = f.size.max(pos+n);
                f.times[1..].fill(Stamp::now()); self.open[desc].position = (pos+n) as i64;
                Ok(n as i64)
            }
            8 => {
                let delta = i64::from_le_bytes(input.try_into().map_err(|_| -22)?);
                let base = match flags { 0 => 0, 1 => d.position, 2 => f.size as i64, _ => return Err(-22) };
                let new = base.checked_add(delta).ok_or(-75)?;
                if new < 0 { return Err(-22); }
                self.open[desc].position = new; Ok(new)
            }
            9 => {
                if flags > 2 { return Err(-22); }
                let new = if flags == 2 {
                    if !(u32::from(BASE)..u32::from(BASE)+FDS as u32).contains(&arg) { return Err(-9); }
                    if u32::from(fd) == arg { return Ok(i64::from(fd)); }
                    arg as usize - usize::from(BASE)
                } else {
                    if arg >= u32::from(BASE) + FDS as u32 { return Err(-22); }
                    self.handles[id].iter().enumerate().skip((arg as usize).saturating_sub(BASE.into()))
                        .find(|(_, h)| **h == 0).map(|(i, _)| i).ok_or(-24)?
                };
                self.handles[id][new] = (desc + 1) as u8 | if flags == 1 { 128 } else { 0 };
                self.reap(); Ok(i64::from(BASE) + new as i64)
            }
            10 if flags == 0 => Ok(i64::from(self.handles[id][slot] >> 7)),
            10 if flags == 1 && arg <= 1 => {
                self.handles[id][slot] = (desc + 1) as u8 | (arg << 7) as u8; Ok(0)
            }
            _ => Err(-22),
        }
    }
    fn info(&self, node: usize) -> (u32, u16, u16, u32, [Stamp; 3]) {
        let f = &self.files[node];
        (f.ino, 0o100000 | f.mode, u16::from(f.length != 0), f.size as u32, f.times)
    }
}

#[derive(Clone, Copy)]
struct Exchange { data: [u8; PAYLOAD], used: usize, length: usize, reply: usize, active: bool }
impl Exchange {
    const EMPTY: Self = Self { data: [0; PAYLOAD], used: 0, length: 0, reply: 0, active: false };
    fn stat(&mut self, info: (u32, u16, u16, u32, [Stamp; 3])) -> Result<i64, i32> {
        let (ino, mode, links, size, times) = info;
        if times.iter().any(|t| !t.valid) { return Err(-61); }
        self.data[..4].copy_from_slice(&ino.to_le_bytes());
        self.data[4..6].copy_from_slice(&mode.to_le_bytes());
        self.data[6..8].copy_from_slice(&links.to_le_bytes());
        self.data[8..12].copy_from_slice(&size.to_le_bytes());
        for (i, t) in times.iter().enumerate() {
            let start = 12 + i * 12;
            self.data[start..start+8].copy_from_slice(&t.seconds.to_le_bytes());
            self.data[start+8..start+12].copy_from_slice(&t.micros.to_le_bytes());
        }
        self.reply = 48; Ok(0)
    }
}

pub(super) struct Mount {
    banks: [Fs; 2], live: usize, clients: [Exchange; CLIENTS], rights: u64,
}
impl Mount {
    const EMPTY: Self = Self { banks: [Fs::EMPTY; 2], live: 0, clients: [Exchange::EMPTY; CLIENTS], rights: 0 };
    pub(super) fn init(&mut self) {
        let rc: i64;
        unsafe { asm!("svc #0x4a01", in("x8") 204u64, inout("x0") 0u64 => rc,
                      inout("x1") 0u64 => _, inout("x2") 0u64 => _, options(nostack)) };
        if rc > 0 {
            self.rights = rc as u64; let now = Stamp::now();
            self.banks[0].root_time = now; self.banks[0].dir_times = [now; 3];
        }
    }
    pub(super) fn exit(&mut self, id: usize) {
        let fs = &mut self.banks[self.live];
        fs.handles[id] = [0; FDS]; fs.reap(); self.clients[id] = Exchange::EMPTY;
    }
    fn trial(&mut self) -> usize {
        let (a, b) = self.banks.split_at_mut(1);
        if self.live == 0 { b.copy_from_slice(a); } else { a.copy_from_slice(b); }
        1 - self.live
    }
}
#[link_section = ".data.tmpfs_service"]
pub(super) static mut MOUNT: Mount = Mount::EMPTY;

#[no_mangle]
#[inline(never)]
pub(super) fn jnox_tmpfs_call(m: &mut Mount, id: usize, token: u64, word: u64) -> Result<(), i64> {
    let rights = ((m.rights >> (id * 2)) & 3) as u8;
    let tag = word >> 56;
    let args = word & 0x00ff_ffff_ffff_ffff;
    let mut x = m.clients[id];
    let mut publish = None;
    let result = if rights == 0 { Err(-13) } else { match tag {
        0x70 if args <= PAYLOAD as u64 => {
            x = Exchange { length: args as usize, active: true, ..Exchange::EMPTY }; Ok(0)
        }
        0x71 if x.active && (args >> 48) as usize == x.used && x.used < x.length => {
            let n = (x.length - x.used).min(6); let data = args.to_le_bytes();
            if data[n..6].iter().any(|v| *v != 0) { Err(-22) } else {
                x.data[x.used..x.used+n].copy_from_slice(&data[..n]); x.used += n; Ok(0)
            }
        }
        0x72 if x.active && x.used == x.length => {
            let bank = m.trial(); x.reply = 0; x.active = false;
            let result = m.banks[bank].operate(id, rights, word, &mut x);
            if result.is_ok() { publish = Some(bank); } result
        }
        0x73 if !x.active && args < x.reply as u64 => {
            let at = args as usize; let n = (x.reply - at).min(6); let mut bytes = [0u8; 8];
            bytes[..n].copy_from_slice(&x.data[at..at+n]); Ok(i64::from_le_bytes(bytes))
        }
        _ => Err(-22)
    }};
    match ipc::reply(token, result.unwrap_or_else(i64::from) as u64) {
        Ok(()) => { m.clients[id] = x; if let Some(bank) = publish { m.live = bank; } Ok(()) }
        Err(-116) => Ok(()),
        Err(rc) => Err(rc),
    }
}
