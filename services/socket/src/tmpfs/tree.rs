//! Fixed directory entries and separately allocated inodes. An open description
//! can keep either kind of inode alive after its final name disappears.
use super::{Fs, Stamp, FILES};

pub(super) const DIRS: usize = 6;
const NAMES: usize = 12;
const COMPONENT: usize = 31;
pub(super) const PATH_BYTES: usize = 63;
const ROOT: usize = 0;
const TMP: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Object { File(usize), Dir(usize) }

#[derive(Clone, Copy)]
struct Name { len: usize, bytes: [u8; COMPONENT] }
impl Name {
    const EMPTY: Self = Self { len: 0, bytes: [0; COMPONENT] };
    fn new(raw: &[u8]) -> Result<Self, i32> {
        if raw.is_empty() || raw == b"." || raw == b".." || raw.contains(&0) { return Err(-22); }
        if raw.len() > COMPONENT { return Err(-36); }
        let mut name = Self::EMPTY; name.len = raw.len();
        name.bytes[..raw.len()].copy_from_slice(raw); Ok(name)
    }
    fn matches(&self, other: &Self) -> bool {
        self.len == other.len && self.bytes[..self.len] == other.bytes[..other.len]
    }
}
#[derive(Clone, Copy)]
struct Entry { name: Name, parent: usize, object: Object }
impl Entry {
    const EMPTY: Self = Self { name: Name::EMPTY, parent: 0, object: Object::Dir(0) };
}
#[derive(Clone, Copy)]
pub(super) struct Dir { pub(super) ino: u32, pub(super) mode: u16, pub(super) times: [Stamp; 3] }
impl Dir {
    const EMPTY: Self = Self { ino: 0, mode: 0, times: [Stamp::UNSET; 3] };
}
#[derive(Clone, Copy)]
pub(super) struct Tree { entries: [Entry; NAMES], pub(super) dirs: [Dir; DIRS] }
#[derive(Clone, Copy)]
pub(super) struct Location { pub(super) parent: usize, name: Name }

impl Tree {
    pub(super) const EMPTY: Self = Self { entries: [Entry::EMPTY; NAMES], dirs: [Dir::EMPTY; DIRS] };
    pub(super) fn init(&mut self, stamp: Stamp) {
        self.dirs[ROOT] = Dir { ino: 1, mode: 0o555, times: [stamp; 3] };
        self.dirs[TMP] = Dir { ino: 2, mode: 0o1777, times: [stamp; 3] };
        let mut name = Name::EMPTY; name.len = 3; name.bytes[..3].copy_from_slice(b"tmp");
        self.entries[0] = Entry { name, parent: ROOT, object: Object::Dir(TMP) };
    }
    fn index(&self, l: Location) -> Option<usize> {
        self.entries.iter().position(|e| e.name.len != 0 && e.parent == l.parent && e.name.matches(&l.name))
    }
    pub(super) fn get(&self, l: Location) -> Option<Object> {
        self.index(l).map(|i| self.entries[i].object)
    }
    pub(super) fn location(&self, raw: &[u8], mutating: bool) -> Result<Location, i32> {
        if raw.len() > PATH_BYTES { return Err(-36); }
        if raw.is_empty() { return Err(-2); }
        if raw[0] != b'/' { return Err(-22); }
        if raw == b"/" { return Err(if mutating { -30 } else { -21 }); }
        let mut components = raw[1..].split(|b| *b == b'/').peekable();
        let mut parent = ROOT;
        while let Some(part) = components.next() {
            let name = Name::new(part)?;
            if self.dirs[parent].mode & 0o100 == 0 { return Err(-13); }
            let location = Location { parent, name };
            if parent == ROOT && (part != b"tmp" || (mutating && components.peek().is_none())) {
                return Err(if mutating { -30 } else { -2 });
            }
            if components.peek().is_none() { return Ok(location); }
            parent = match self.get(location) {
                Some(Object::Dir(dir)) => dir, Some(Object::File(_)) => return Err(-20), None => return Err(-2),
            };
        }
        Err(-22)
    }
    pub(super) fn object(&self, raw: &[u8]) -> Result<Object, i32> {
        if raw == b"/" { Ok(Object::Dir(ROOT)) } else {
            self.get(self.location(raw, false)?).ok_or(-2)
        }
    }
    pub(super) fn editable(&self, l: Location) -> Result<(), i32> {
        if l.parent == ROOT { return Err(-30); }
        if self.dirs[l.parent].mode & 0o300 != 0o300 { return Err(-13); }
        Ok(())
    }
    pub(super) fn add(&mut self, l: Location, obj: Object) -> Result<(), i32> {
        self.editable(l)?;
        if self.get(l).is_some() { return Err(-17); }
        let e = self.entries.iter_mut().find(|e| e.name.len == 0).ok_or(-28)?;
        *e = Entry { name: l.name, parent: l.parent, object: obj }; Ok(())
    }
    pub(super) fn links(&self, object: Object) -> u16 {
        self.entries.iter().filter(|e| e.name.len != 0 && e.object == object).count() as u16
    }
    pub(super) fn children(&self, dir: usize) -> impl Iterator<Item=Object> + '_ {
        self.entries.iter().filter(move |e| e.name.len != 0 && e.parent == dir).map(|e| e.object)
    }
    fn contains_dir(&self, ancestor: usize, mut child: usize) -> bool {
        for _ in 0..DIRS {
            if ancestor == child { return true; }
            if child == ROOT { return false; }
            let Some(e) = self.entries.iter().find(|e| e.name.len != 0 && e.object == Object::Dir(child))
                else { return false };
            child = e.parent;
        }
        true
    }
    fn renamed_extent(&self, source: usize, new_length: usize) -> Result<(), i32> {
        for e in self.entries.iter().filter(|e| e.name.len != 0) {
            let mut suffix = 1 + e.name.len; let mut dir = e.parent;
            for _ in 0..DIRS {
                if dir == source {
                    if new_length + suffix > PATH_BYTES { return Err(-36); }
                    break;
                }
                if dir == ROOT { break; }
                let parent = self.entries.iter().find(|p| p.name.len != 0 && p.object == Object::Dir(dir))
                    .ok_or(-5)?;
                suffix += 1 + parent.name.len; dir = parent.parent;
            }
        }
        Ok(())
    }
}

impl Fs {
    pub(super) fn allocate_ino(&mut self) -> Result<u32, i32> {
        let ino = self.next_ino; self.next_ino = ino.checked_add(1).ok_or(-75)?; Ok(ino)
    }
    pub(super) fn namespace(&mut self, op: u8, raw: &[u8], mode: u32) -> Result<i64, i32> {
        if op == 14 {
            let object = self.tree.object(raw)?;
            self.chmod(object, mode)?;
            return Ok(0);
        }
        let (first, second) = if matches!(op, 12|13) {
            let split = raw.iter().position(|b| *b == 0).ok_or(-22)?;
            (&raw[..split], &raw[split+1..])
        } else { (raw, &[][..]) };
        if op == 11 && (first == b"/" || first == b"/tmp") { return Err(-17); }
        let from = self.tree.location(first, op != 13)?;
        let source = self.tree.get(from);
        let now = Stamp::now();
        match op {
            2 => {
                self.tree.editable(from)?;
                let object = source.ok_or(-2)?;
                if matches!(object, Object::Dir(_)) { return Err(-21); }
                let index = self.tree.index(from).ok_or(-2)?;
                self.tree.entries[index] = Entry::EMPTY;
                self.times(object)[2] = now;
            }
            11 => {
                self.tree.editable(from)?;
                if source.is_some() { return Err(-17); }
                if mode > 0o1777 { return Err(-95); }
                let id = (2..DIRS).find(|&i| self.tree.dirs[i].ino == 0).ok_or(-28)?;
                self.tree.add(from, Object::Dir(id))?;
                self.tree.dirs[id] = Dir { ino: self.allocate_ino()?, mode: mode as u16, times: [now; 3] };
            }
            12 | 13 => {
                let source = source.ok_or(-2)?;
                let to = self.tree.location(second, true)?;
                self.tree.editable(to)?;
                if op == 13 {
                    if matches!(source, Object::Dir(_)) { return Err(-1); }
                    self.tree.add(to, source)?;
                    self.times(source)[2] = now;
                    self.tree.dirs[to.parent].times[1..].fill(now);
                    return Ok(0);
                }
                self.tree.editable(from)?;
                let src = self.tree.index(from).ok_or(-2)?;
                let replacement = self.tree.index(to);
                if let Object::Dir(dir) = source {
                    if self.tree.contains_dir(dir, to.parent) { return Err(-22); }
                    self.tree.renamed_extent(dir, second.len())?;
                }
                if let Some(i) = replacement {
                    let old = self.tree.entries[i].object;
                    if old == source { return Ok(0); }
                    match (source, old) {
                        (Object::File(_), Object::Dir(_)) => return Err(-21),
                        (Object::Dir(_), Object::File(_)) => return Err(-20),
                        (_, Object::Dir(d)) if self.tree.children(d).next().is_some() => return Err(-39),
                        _ => {}
                    }
                    self.times(old)[2] = now; self.tree.entries[i] = Entry::EMPTY;
                }
                self.tree.entries[src] = Entry { name: to.name, parent: to.parent, object: source };
                self.times(source)[2] = now; self.tree.dirs[to.parent].times[1..].fill(now);
            }
            _ => return Err(-22),
        }
        self.tree.dirs[from.parent].times[1..].fill(now);
        self.reap(); Ok(0)
    }
    pub(super) fn times(&mut self, object: Object) -> &mut [Stamp; 3] {
        match object { Object::File(i) => &mut self.files[i].times, Object::Dir(i) => &mut self.tree.dirs[i].times }
    }
    pub(super) fn chmod(&mut self, object: Object, mode: u32) -> Result<(), i32> {
        if mode > 0o1777 { return Err(-95); }
        match object {
            Object::Dir(ROOT) => return Err(-30),
            Object::Dir(i) => self.tree.dirs[i].mode = mode as u16,
            Object::File(i) => {
                if mode > 0o777 { return Err(-95); }
                self.files[i].mode = mode as u16;
            }
        }
        self.times(object)[2] = Stamp::now(); Ok(())
    }
    pub(super) fn metadata(&self, object: Object) -> (u32,u16,u16,u32,[Stamp;3]) {
        let links = self.tree.links(object);
        match object {
            Object::File(i) => {
                let f = &self.files[i];
                (f.ino, 0o100000 | f.mode, links, f.size as u32, f.times)
            }
            Object::Dir(i) => {
                let d = self.tree.dirs[i];
                let n = if i != ROOT && links == 0 { 0 } else {
                    2 + self.tree.children(i).filter(|o| matches!(o, Object::Dir(_))).count() as u16
                };
                (d.ino, 0o40000 | d.mode, n, 0, d.times)
            }
        }
    }
    pub(super) fn reclaim_inodes(&mut self) {
        for i in 0..FILES {
            let obj = Object::File(i);
            if self.tree.links(obj) == 0 && !self.open.iter().any(|d| d.flags != 0 && d.object == obj) {
                self.files[i].ino = 0;
            }
        }
        for i in 2..DIRS {
            let obj = Object::Dir(i);
            if self.tree.links(obj) == 0 && !self.open.iter().any(|d| d.flags != 0 && d.object == obj) {
                self.tree.dirs[i] = Dir::EMPTY;
            }
        }
    }
}
