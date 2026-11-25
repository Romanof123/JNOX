//! Serialized native pipe/FIFO state. Syscall adapters validate private user
//! spans before entry, retry a blocked I/O on readiness, and deliver -EPIPE
//! directly in this profile (signal delivery belongs to the signal subsystem).
pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;
pub const O_NONBLOCK: u32 = 0x800;
pub const O_CLOEXEC: u32 = 0x80000;
pub const FD_CLOEXEC: u32 = 1;
pub const CAPACITY: usize = 2048;
pub const PIPE_BUF: usize = 512;
pub const OBJECTS: usize = 4;
const FD_LIMIT: usize = 8;
const FIRST_AUTO: usize = 3;
const FD_NUMBERS: usize = FIRST_AUTO + FD_LIMIT;
const DESCRIPTIONS: usize = 3 * FD_LIMIT;
const NAME_LIMIT: usize = 31;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Name {
    bytes: [u8; NAME_LIMIT],
    len: usize,
}

impl Name {
    fn parse(path: &[u8]) -> Result<Self, i32> {
        if path.is_empty() {
            return Err(-2);
        }
        if path.len() > NAME_LIMIT {
            return Err(-36);
        }
        if path.contains(&0) {
            return Err(-22);
        }
        let mut name = Self {
            bytes: [0; NAME_LIMIT],
            len: path.len(),
        };
        name.bytes[..path.len()].copy_from_slice(path);
        Ok(name)
    }
}

#[derive(Clone, Copy)]
struct Fd {
    description: usize,
    cloexec: bool,
}

#[derive(Clone, Copy)]
struct Description {
    object: usize,
    generation: u64,
    writer: bool,
    nonblock: bool,
    refs: usize,
}

#[derive(Clone, Copy)]
struct Opening {
    fd: usize,
    ready: bool,
}

#[derive(Clone, Copy)]
struct Process {
    alive: bool,
    fds: [Option<Fd>; FD_NUMBERS],
    opening: Option<Opening>,
}

impl Process {
    const NEW: Self = Self {
        alive: true,
        fds: [None; FD_NUMBERS],
        opening: None,
    };

    fn auto_fds<const K: usize>(&self) -> Result<[usize; K], i32> {
        if self.fds.iter().flatten().count() + K > FD_LIMIT {
            return Err(-24);
        }
        let mut available = (FIRST_AUTO..FD_NUMBERS).filter(|&fd| self.fds[fd].is_none());
        let mut result = [0; K];
        for fd in &mut result {
            *fd = available.next().ok_or(-24)?;
        }
        Ok(result)
    }
}

struct Pipe {
    allocated: bool,
    generation: u64,
    name: Option<Name>,
    owner: usize,
    reader_mask: u8,
    writer_mask: u8,
    readers: usize,
    writers: usize,
    bytes: [u8; CAPACITY],
    head: usize,
    len: usize,
    peak: usize, // lifetime high water of this physical object slot
}

impl Pipe {
    const EMPTY: Self = Self {
        allocated: false,
        generation: 0,
        name: None,
        owner: 0,
        reader_mask: 0,
        writer_mask: 0,
        readers: 0,
        writers: 0,
        bytes: [0; CAPACITY],
        head: 0,
        len: 0,
        peak: 0,
    };

    fn read(&mut self, dst: &mut [u8]) -> usize {
        let n = dst.len().min(self.len);
        let first = n.min(CAPACITY - self.head);
        dst[..first].copy_from_slice(&self.bytes[self.head..self.head + first]);
        dst[first..n].copy_from_slice(&self.bytes[..n - first]);
        self.head = (self.head + n) % CAPACITY;
        self.len -= n;
        n
    }

    fn write(&mut self, src: &[u8]) -> usize {
        let n = src.len().min(CAPACITY - self.len);
        let tail = (self.head + self.len) % CAPACITY;
        let first = n.min(CAPACITY - tail);
        self.bytes[tail..tail + first].copy_from_slice(&src[..first]);
        self.bytes[..n - first].copy_from_slice(&src[first..n]);
        self.len += n;
        self.peak = self.peak.max(self.len);
        n
    }

    fn writable(&self, n: usize) -> bool {
        let space = CAPACITY - self.len;
        n == 0 || self.readers == 0 || if n <= PIPE_BUF { space >= n } else { space != 0 }
    }

    fn retire_if_unused(&mut self) {
        if self.readers == 0 && self.writers == 0 {
            self.bytes.fill(0);
            self.head = 0;
            self.len = 0;
            self.allocated = self.name.is_some();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipeStats {
    pub generation: u64,
    pub allocated: bool,
    pub linked: bool,
    pub readers: usize,
    pub writers: usize,
    pub fd_refs: usize,
    pub bytes: usize,
    pub high_water: usize,
}

pub struct PipeIpc<const N: usize> {
    pipes: [Pipe; OBJECTS],
    descriptions: [Option<Description>; DESCRIPTIONS],
    processes: [Process; N],
}

impl<const N: usize> PipeIpc<N> {
    pub const fn new() -> Self {
        assert!(N >= 1 && N <= 3);
        Self {
            pipes: [const { Pipe::EMPTY }; OBJECTS],
            descriptions: [None; DESCRIPTIONS],
            processes: [Process::NEW; N],
        }
    }

    fn process(&self, pid: usize) -> Result<&Process, i32> {
        self.processes.get(pid).filter(|p| p.alive).ok_or(-3)
    }

    fn fd(&self, pid: usize, fd: usize) -> Result<Fd, i32> {
        let process = self.process(pid)?;
        if process.opening.is_some_and(|open| open.fd == fd) {
            return Err(-9);
        }
        process.fds.get(fd).copied().flatten().ok_or(-9)
    }

    fn description(&self, pid: usize, fd: usize) -> Result<Description, i32> {
        let d = self.descriptions[self.fd(pid, fd)?.description].unwrap();
        debug_assert_eq!(d.generation, self.pipes[d.object].generation);
        Ok(d)
    }

    fn free_descriptions<const K: usize>(&self) -> Result<[usize; K], i32> {
        let mut free = self.descriptions.iter().enumerate().filter(|(_, d)| d.is_none());
        let mut result = [0; K];
        for id in &mut result {
            *id = free.next().ok_or(-23)?.0;
        }
        Ok(result)
    }

    fn allocate(&mut self, owner: usize, name: Option<Name>, readers: u8, writers: u8) -> Result<usize, i32> {
        let index = self
            .pipes
            .iter()
            .position(|p| !p.allocated && p.generation != u64::MAX)
            .ok_or(-23)?;
        let p = &mut self.pipes[index];
        p.allocated = true;
        p.generation += 1;
        p.owner = owner;
        p.name = name;
        p.reader_mask = readers;
        p.writer_mask = writers;
        Ok(index)
    }

    fn install(&mut self, pid: usize, fd: usize, id: usize, object: usize, writer: bool, flags: u32) {
        let p = &mut self.pipes[object];
        if writer {
            p.writers += 1;
        } else {
            p.readers += 1;
        }
        self.descriptions[id] = Some(Description {
            object,
            generation: p.generation,
            writer,
            nonblock: flags & O_NONBLOCK != 0,
            refs: 1,
        });
        self.processes[pid].fds[fd] = Some(Fd {
            description: id,
            cloexec: flags & O_CLOEXEC != 0,
        });
    }

    pub fn pipe(&mut self, pid: usize, flags: u32) -> Result<(usize, usize), i32> {
        if flags & !(O_NONBLOCK | O_CLOEXEC) != 0 {
            return Err(-22);
        }
        let [read, write] = self.process(pid)?.auto_fds()?;
        let [r, w] = self.free_descriptions()?;
        let object = self.allocate(pid, None, 0, 0)?;
        self.install(pid, read, r, object, false, flags);
        self.install(pid, write, w, object, true, flags);
        Ok((read, write))
    }

    /// Install both sides of one manifest-authorized anonymous pipe atomically.
    /// No temporary FDs are needed, even in a nearly full process table.
    pub fn provision_link(
        &mut self,
        writer_pid: usize,
        write_fd: usize,
        reader_pid: usize,
        read_fd: usize,
    ) -> Result<(), i32> {
        if (writer_pid, write_fd) == (reader_pid, read_fd) {
            return Err(-22);
        }
        let needed = if writer_pid == reader_pid { 2 } else { 1 };
        for (pid, fd) in [(writer_pid, write_fd), (reader_pid, read_fd)] {
            let process = self.process(pid)?;
            if fd >= FD_NUMBERS {
                return Err(-9);
            }
            if process.fds[fd].is_some() {
                return Err(-16);
            }
            if process.fds.iter().flatten().count() + needed > FD_LIMIT {
                return Err(-24);
            }
        }
        let [r, w] = self.free_descriptions()?;
        let object = self.allocate(writer_pid, None, 0, 0)?;
        self.install(writer_pid, write_fd, w, object, true, 0);
        self.install(reader_pid, read_fd, r, object, false, 0);
        Ok(())
    }

    fn named(&self, name: Name) -> Option<usize> {
        self.pipes.iter().position(|p| p.name == Some(name))
    }

    /// The caller supplies reader/writer PID grants checked by the native adapter.
    pub fn mkfifo(&mut self, owner: usize, path: &[u8], readers: u8, writers: u8) -> Result<(), i32> {
        self.process(owner)?;
        let name = Name::parse(path)?;
        if (readers | writers) >> N != 0 {
            return Err(-22);
        }
        if self.named(name).is_some() {
            return Err(-17);
        }
        self.allocate(owner, Some(name), readers, writers).map(|_| ())
    }

    pub fn unlink(&mut self, owner: usize, path: &[u8]) -> Result<(), i32> {
        self.process(owner)?;
        let index = self.named(Name::parse(path)?).ok_or(-2)?;
        let pipe = &mut self.pipes[index];
        if pipe.owner != owner {
            return Err(-1);
        }
        pipe.name = None;
        pipe.retire_if_unused();
        Ok(())
    }

    /// A blocked opener already owns a reserved descriptor and pins this FIFO
    /// instance. The opposing open latches success even if it closes immediately.
    pub fn open(&mut self, pid: usize, path: &[u8], flags: u32) -> Result<Option<usize>, i32> {
        if self.process(pid)?.opening.is_some() {
            return Err(-16);
        }
        if flags & !(O_WRONLY | O_NONBLOCK | O_CLOEXEC) != 0 {
            return Err(-22);
        }
        let index = self.named(Name::parse(path)?).ok_or(-2)?;
        let p = &self.pipes[index];
        let writer = flags & O_WRONLY != 0;
        let allowed = if writer { p.writer_mask } else { p.reader_mask };
        if allowed & (1 << pid) == 0 {
            return Err(-13);
        }
        let peer = if writer { p.readers != 0 } else { p.writers != 0 };
        let nonblock = flags & O_NONBLOCK != 0;
        if nonblock && writer && !peer {
            return Err(-6);
        }
        let [fd] = self.processes[pid].auto_fds()?;
        let [id] = self.free_descriptions()?;
        self.install(pid, fd, id, index, writer, flags);
        if !peer && !nonblock {
            self.processes[pid].opening = Some(Opening { fd, ready: false });
            return Ok(None);
        }
        if peer {
            for p in &mut self.processes {
                if let Some(opening) = &mut p.opening {
                    let entry = p.fds[opening.fd].unwrap();
                    let other = self.descriptions[entry.description].unwrap();
                    if other.object == index && other.writer != writer {
                        opening.ready = true;
                    }
                }
            }
        }
        Ok(Some(fd))
    }

    pub fn open_ready(&self, pid: usize) -> bool {
        self.processes.get(pid).and_then(|p| p.opening).is_some_and(|o| o.ready)
    }

    pub fn take_open(&mut self, pid: usize) -> Option<usize> {
        if !self.open_ready(pid) {
            return None;
        }
        self.processes[pid].opening.take().map(|o| o.fd)
    }

    fn endpoint(&self, pid: usize, fd: usize, writer: bool) -> Result<Description, i32> {
        let d = self.description(pid, fd)?;
        if d.writer != writer {
            return Err(-9);
        }
        Ok(d)
    }

    pub fn read_ready(&self, pid: usize, fd: usize, n: usize) -> Result<bool, i32> {
        let p = &self.pipes[self.endpoint(pid, fd, false)?.object];
        Ok(n == 0 || p.len != 0 || p.writers == 0)
    }

    pub fn write_ready(&self, pid: usize, fd: usize, n: usize) -> Result<bool, i32> {
        Ok(self.pipes[self.endpoint(pid, fd, true)?.object].writable(n))
    }

    /// Side-effect-free prediction under the same serialized scheduler boundary
    /// as the subsequent copy/commit. Empty read + last writer gone yields EOF.
    pub fn read_span(&self, pid: usize, fd: usize, n: usize) -> Result<Option<usize>, i32> {
        let d = self.endpoint(pid, fd, false)?;
        let p = &self.pipes[d.object];
        if n == 0 || p.len != 0 || p.writers == 0 {
            return Ok(Some(n.min(p.len)));
        }
        if d.nonblock { Err(-11) } else { Ok(None) }
    }

    pub fn write_span(&self, pid: usize, fd: usize, n: usize) -> Result<Option<usize>, i32> {
        let d = self.endpoint(pid, fd, true)?;
        let p = &self.pipes[d.object];
        if n == 0 {
            return Ok(Some(0));
        }
        if p.readers == 0 {
            return Err(-32);
        }
        if p.writable(n) {
            return Ok(Some(n.min(CAPACITY - p.len)));
        }
        if d.nonblock { Err(-11) } else { Ok(None) }
    }

    pub fn read(&mut self, pid: usize, fd: usize, dst: &mut [u8]) -> Result<Option<usize>, i32> {
        let Some(n) = self.read_span(pid, fd, dst.len())? else {
            return Ok(None);
        };
        let object = self.description(pid, fd)?.object;
        Ok(Some(self.pipes[object].read(&mut dst[..n])))
    }

    pub fn write(&mut self, pid: usize, fd: usize, src: &[u8]) -> Result<Option<usize>, i32> {
        let Some(n) = self.write_span(pid, fd, src.len())? else {
            return Ok(None);
        };
        let object = self.description(pid, fd)?.object;
        Ok(Some(self.pipes[object].write(&src[..n])))
    }

    pub fn getfl(&self, pid: usize, fd: usize) -> Result<u32, i32> {
        let d = self.description(pid, fd)?;
        Ok((if d.writer { O_WRONLY } else { O_RDONLY }) | (if d.nonblock { O_NONBLOCK } else { 0 }))
    }

    pub fn getfd(&self, pid: usize, fd: usize) -> Result<u32, i32> {
        Ok(if self.fd(pid, fd)?.cloexec { FD_CLOEXEC } else { 0 })
    }

    pub fn set_nonblock(&mut self, pid: usize, fd: usize, value: bool) -> Result<(), i32> {
        let id = self.fd(pid, fd)?.description;
        self.descriptions[id].as_mut().unwrap().nonblock = value;
        Ok(())
    }

    pub fn set_cloexec(&mut self, pid: usize, fd: usize, value: bool) -> Result<(), i32> {
        self.fd(pid, fd)?;
        self.processes[pid].fds[fd].as_mut().unwrap().cloexec = value;
        Ok(())
    }

    pub fn dup(&mut self, pid: usize, fd: usize) -> Result<usize, i32> {
        let old = self.fd(pid, fd)?;
        let [target] = self.processes[pid].auto_fds()?;
        self.duplicate(old, pid, target, false)?;
        Ok(target)
    }

    pub fn dup2(&mut self, pid: usize, fd: usize, target: usize) -> Result<usize, i32> {
        let old = self.fd(pid, fd)?;
        if fd != target {
            self.duplicate(old, pid, target, false)?;
        }
        Ok(target)
    }

    /// Trusted launcher operation. A user-supplied PID cannot authorize it.
    pub fn inherit(&mut self, src_pid: usize, src_fd: usize, dst_pid: usize, dst_fd: usize) -> Result<(), i32> {
        let old = self.fd(src_pid, src_fd)?;
        if (src_pid, src_fd) != (dst_pid, dst_fd) {
            self.duplicate(old, dst_pid, dst_fd, old.cloexec)?;
        }
        Ok(())
    }

    fn duplicate(&mut self, old: Fd, pid: usize, target: usize, cloexec: bool) -> Result<(), i32> {
        let process = self.process(pid)?;
        if target >= FD_NUMBERS {
            return Err(-9);
        }
        if process.opening.is_some_and(|o| o.fd == target) {
            return Err(-16);
        }
        if process.fds[target].is_none() && process.fds.iter().flatten().count() == FD_LIMIT {
            return Err(-24);
        }
        self.remove_fd(pid, target);
        self.descriptions[old.description].as_mut().unwrap().refs += 1;
        self.processes[pid].fds[target] = Some(Fd {
            description: old.description,
            cloexec,
        });
        Ok(())
    }

    fn remove_fd(&mut self, pid: usize, fd: usize) {
        let Some(entry) = self.processes[pid].fds[fd].take() else {
            return;
        };
        let id = entry.description;
        let d = self.descriptions[id].as_mut().unwrap();
        d.refs -= 1;
        if d.refs == 0 {
            let p = &mut self.pipes[d.object];
            if d.writer {
                p.writers -= 1;
            } else {
                p.readers -= 1;
            }
            self.descriptions[id] = None;
            p.retire_if_unused();
        }
    }

    pub fn close(&mut self, pid: usize, fd: usize) -> Result<(), i32> {
        self.fd(pid, fd)?;
        self.remove_fd(pid, fd);
        Ok(())
    }

    pub fn exit(&mut self, pid: usize) {
        if self.process(pid).is_err() {
            return;
        }
        self.processes[pid].opening = None;
        for fd in 0..FD_NUMBERS {
            self.remove_fd(pid, fd);
        }
        self.processes[pid].alive = false;
    }

    pub fn snapshot(&self, slot: usize) -> Option<PipeStats> {
        let p = self.pipes.get(slot)?;
        Some(PipeStats {
            generation: p.generation,
            allocated: p.allocated,
            linked: p.name.is_some(),
            readers: p.readers,
            writers: p.writers,
            bytes: p.len,
            high_water: p.peak,
            fd_refs: self
                .descriptions
                .iter()
                .flatten()
                .filter(|d| d.object == slot)
                .map(|d| d.refs)
                .sum(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_stream_two_writers_short_atomic_rejection_partial_large_writes() {
        let mut core = PipeIpc::<3>::new();
        core.provision_link(0, 1, 2, 0).unwrap();
        core.inherit(0, 1, 1, 7).unwrap();
        core.set_nonblock(1, 7, true).unwrap();
        assert_eq!(core.write(0, 1, &[0xa1; CAPACITY - 1]), Ok(Some(CAPACITY - 1)));
        assert_eq!(core.write_ready(1, 7, PIPE_BUF), Ok(false));
        assert_eq!(core.write(1, 7, &[0xb2; PIPE_BUF]), Err(-11));
        assert_eq!(core.write_span(1, 7, usize::MAX), Ok(Some(1)));
        assert_eq!(core.write(1, 7, &[0xc3; PIPE_BUF + 1]), Ok(Some(1)));
        let mut first = [0; PIPE_BUF];
        assert_eq!(core.read(2, 0, &mut first[..511]), Ok(Some(511)));
        assert_eq!(core.write(1, 7, &[0xb2; PIPE_BUF]), Err(-11));
        assert_eq!(core.read(2, 0, &mut first[511..]), Ok(Some(1)));
        assert_eq!(first, [0xa1; PIPE_BUF]);
        assert_eq!(core.write(1, 7, &[0xb2; PIPE_BUF]), Ok(Some(PIPE_BUF)));
        let mut rest = [0; CAPACITY];
        assert_eq!(core.read(2, 0, &mut rest), Ok(Some(CAPACITY)));
        assert!(rest[..1535].iter().all(|&x| x == 0xa1));
        assert_eq!(rest[1535], 0xc3);
        assert!(rest[1536..].iter().all(|&x| x == 0xb2));
        assert_eq!(core.snapshot(0).unwrap().high_water, CAPACITY);
        core.exit(0);
        assert_eq!(core.read(2, 0, &mut first), Ok(None));
        core.exit(1);
        assert_eq!(core.read_ready(2, 0, 1), Ok(true));
        assert_eq!(core.read(2, 0, &mut first), Ok(Some(0)));
        core.exit(2);
        assert!(!core.snapshot(0).unwrap().allocated);
    }

    #[test]
    fn descriptor_flags_and_last_reference_edges() {
        let mut core = PipeIpc::<2>::new();
        let (r, w) = core.pipe(0, O_CLOEXEC).unwrap();
        assert_eq!((r, w), (3, 4));
        assert_eq!(core.dup2(0, r, r), Ok(r));
        assert_eq!(core.getfd(0, r), Ok(FD_CLOEXEC));
        let copy = core.dup(0, r).unwrap();
        assert_eq!(core.getfd(0, copy), Ok(0));
        core.inherit(0, r, 1, 0).unwrap();
        assert_eq!(core.getfd(1, 0), Ok(FD_CLOEXEC));
        core.set_cloexec(1, 0, false).unwrap();
        assert_eq!(core.getfd(0, r), Ok(FD_CLOEXEC));
        core.set_nonblock(1, 0, true).unwrap();
        assert_eq!(core.getfl(0, copy), Ok(O_NONBLOCK));
        assert_eq!(core.getfl(0, w), Ok(O_WRONLY));
        assert_eq!(core.snapshot(0).unwrap().readers, 1);
        assert_eq!(core.snapshot(0).unwrap().fd_refs, 4);
        assert_eq!(core.dup2(0, 10, w), Err(-9)); // keep target intact
        assert_eq!(core.write(0, w, b"abc"), Ok(Some(3)));
        core.dup2(0, r, w).unwrap(); // remove last writer
        assert_eq!(core.getfd(0, w), Ok(0));
        assert_eq!(core.snapshot(0).unwrap().writers, 0);
        let mut data = [0; 4];
        assert_eq!(core.read(1, 0, &mut data), Ok(Some(3)));
        assert_eq!(&data[..3], b"abc");
        assert_eq!(core.read(0, copy, &mut data), Ok(Some(0)));
        assert_eq!(core.write(0, w, &[]), Err(-9)); // wrong direction, even when empty
        assert_eq!(core.read(1, 3, &mut []), Err(-9)); // same integer in a different process
    }

    #[test]
    fn fifo_rendezvous_latches_and_masks_match_open_descriptions() {
        let mut core = PipeIpc::<3>::new();
        core.mkfifo(0, b"jobs", 0b001, 0b110).unwrap();
        assert_eq!(core.unlink(1, b"jobs"), Err(-1));
        assert_eq!(core.open(1, b"jobs", O_RDONLY), Err(-13));
        assert_eq!(core.open(0, b"jobs", 2), Err(-22));
        assert_eq!(core.open(1, b"jobs", O_WRONLY | O_NONBLOCK), Err(-6));
        assert_eq!(core.open(1, b"jobs", O_WRONLY), Ok(None));
        assert_eq!(core.open(2, b"jobs", O_WRONLY), Ok(None));
        assert!(!core.open_ready(1));
        assert_eq!(core.take_open(1), None);
        let reader = core.open(0, b"jobs", O_RDONLY | O_NONBLOCK).unwrap().unwrap();
        assert_eq!(core.read(0, reader, &mut [0]), Err(-11));
        core.close(0, reader).unwrap(); // already matched both blocked opens
        for pid in [1, 2] {
            assert!(core.open_ready(pid));
            let writer = core.take_open(pid).unwrap();
            assert_eq!(core.take_open(pid), None);
            assert_eq!(core.write_ready(pid, writer, 512), Ok(true)); // broken pipe
            assert_eq!(core.write(pid, writer, &[]), Ok(Some(0)));
            assert_eq!(core.write(pid, writer, b"x"), Err(-32));
        }
        core.exit(1);
        core.exit(2);
        let p = core.snapshot(0).unwrap();
        assert!(p.linked && p.allocated && p.fd_refs == 0);
        core.unlink(0, b"jobs").unwrap();
        assert!(!core.snapshot(0).unwrap().allocated);
    }

    #[test]
    fn unlink_recreate_keeps_old_waits_old_data_and_new_name_separate() {
        let mut core = PipeIpc::<3>::new();
        core.mkfifo(2, b"q", 7, 7).unwrap();
        assert_eq!(core.open(0, b"q", O_RDONLY), Ok(None));
        core.unlink(2, b"q").unwrap();
        core.mkfifo(2, b"q", 7, 7).unwrap();
        assert_eq!(core.open(1, b"q", O_WRONLY | O_NONBLOCK), Err(-6));
        let r = core.open(1, b"q", O_NONBLOCK).unwrap().unwrap();
        let w = core.open(2, b"q", O_WRONLY).unwrap().unwrap();
        assert!(!core.open_ready(0));
        core.exit(0);
        core.write(2, w, b"old").unwrap();
        core.unlink(2, b"q").unwrap();
        core.mkfifo(2, b"q", 7, 7).unwrap(); // reuses the retired first slot
        assert_eq!(core.snapshot(0).unwrap().generation, 2);
        let fresh = core.open(1, b"q", O_NONBLOCK).unwrap().unwrap();
        assert_eq!(core.read(1, fresh, &mut [0; 3]), Ok(Some(0)));
        let mut old = [0; 3];
        assert_eq!(core.read(1, r, &mut old), Ok(Some(3)));
        assert_eq!(&old, b"old");
        core.exit(2); // named node persists, unlinked node drains to EOF
        assert_eq!(core.read(1, r, &mut [0]), Ok(Some(0)));
        assert!(core.snapshot(0).unwrap().linked);
    }

    #[test]
    fn bounds_and_boot_provisioning_are_transactional() {
        let mut core = PipeIpc::<3>::new();
        let (r, w) = core.pipe(0, 0).unwrap();
        for fd in [0, 1, 2, 5, 6] {
            core.dup2(0, r, fd).unwrap();
        }
        // Seven occupied FDs leave room for one final endpoint, not a pair.
        assert_eq!(core.pipe(0, 0), Err(-24));
        assert_eq!(core.provision_link(0, 7, 0, 8), Err(-24));
        assert_eq!(core.provision_link(0, 8, 0, 8), Err(-22));
        assert_eq!(core.provision_link(1, 0, 0, w), Err(-16));
        assert!(!core.snapshot(1).unwrap().allocated);
        core.provision_link(1, 9, 0, 10).unwrap();
        assert_eq!(core.dup2(0, w, 7), Err(-24));
        assert_eq!(core.dup2(0, w, 1), Ok(1)); // replacement permitted at limit
        core.mkfifo(2, b"a", 0, 0).unwrap();
        core.mkfifo(2, b"b", 0, 0).unwrap();
        assert_eq!(core.pipe(1, 0), Err(-23)); // no free pipe, no half-created pair
        assert_eq!(core.getfd(1, 3), Err(-9));
        assert_eq!(core.mkfifo(2, b"a", 0, 0), Err(-17));
        assert_eq!(core.mkfifo(2, b"x", 8, 0), Err(-22));
        assert_eq!(core.mkfifo(2, &[b'x'; 32], 7, 7), Err(-36));
        assert_eq!(core.unlink(2, b"z"), Err(-2));
        core.exit(0);
        assert_eq!(core.write(1, 9, b"x"), Err(-32));
        core.exit(1);
        for id in 0..2 {
            assert_eq!(core.snapshot(id).unwrap().fd_refs, 0);
        }
    }
}
