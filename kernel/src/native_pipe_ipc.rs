//! Single-core fixed-priority adapter. All pipe mutation and readiness checks
//! run under the masked exception boundary. Only the active owner copies data.
use super::{
    Action, COUNT, FRAMES, Fixed, NATIVE_PIPE_LINKS, NATIVE_PIPE_READERS, NATIVE_PIPE_WRITERS,
    NativeFrame, fail, native_el0_mmu as mmu,
};
#[path = "pipe_ipc.rs"]
mod core_pipe;
use core_pipe::{CAPACITY, FD_CLOEXEC, O_NONBLOCK, O_WRONLY, OBJECTS, PIPE_BUF, PipeIpc};

#[derive(Clone, Copy)]
enum Waiting {
    None,
    Open,
    Read(usize, usize),
    Write(usize, usize),
}

struct Pipes {
    ipc: PipeIpc<COUNT>,
    waits: [Waiting; COUNT],
    blocks: [u64; COUNT],
    wakes: [u64; COUNT],
    read: [u64; COUNT],
    written: [u64; COUNT],
    peaks: [usize; OBJECTS],
    #[cfg(jnox_native_shell)]
    redirected: [u8; COUNT],
}
static mut PIPES: Pipes = Pipes {
    ipc: PipeIpc::new(),
    waits: [Waiting::None; COUNT],
    blocks: [0; COUNT],
    wakes: [0; COUNT],
    read: [0; COUNT],
    written: [0; COUNT],
    peaks: [0; OBJECTS],
    #[cfg(jnox_native_shell)]
    redirected: [0; COUNT],
};

// This accessor confines the mutable borrow to a single handler transaction.
fn with<T>(f: impl FnOnce(&mut Pipes) -> T) -> T {
    unsafe { f(&mut *(&raw mut PIPES)) }
}
fn count(a: &mut u64, n: usize) {
    *a = a.checked_add(n as u64).unwrap_or_else(|| fail(9));
}

pub(super) fn boot() {
    with(|p| {
        for &(wp, wf, rp, rf) in NATIVE_PIPE_LINKS {
            p.ipc
                .provision_link(wp, wf, rp, rf)
                .unwrap_or_else(|_| fail(9));
            #[cfg(jnox_native_shell)]
            {
                if wf < 3 {
                    p.redirected[wp] |= 1 << wf;
                }
                if rf < 3 {
                    p.redirected[rp] |= 1 << rf;
                }
            }
        }
    });
}

impl Pipes {
    fn changed(&mut self, fixed: &mut Fixed, issuer: usize) {
        for i in 0..OBJECTS {
            self.peaks[i] = self.peaks[i].max(self.ipc.snapshot(i).unwrap().high_water);
        }
        for pid in 0..COUNT {
            if pid == issuer {
                continue;
            }
            let ready = match self.waits[pid] {
                Waiting::None => false,
                Waiting::Open => match self.ipc.take_open(pid) {
                    Some(fd) => {
                        // The core has committed this reserved descriptor. The
                        // blocked open resumes after SVC without a second open.
                        unsafe {
                            FRAMES[pid].x[0] = fd as u64;
                        }
                        true
                    }
                    None => false,
                },
                Waiting::Read(fd, n) => self.ipc.read_ready(pid, fd, n).unwrap_or(true),
                Waiting::Write(fd, n) => self.ipc.write_ready(pid, fd, n).unwrap_or(true),
            };
            if ready {
                self.waits[pid] = Waiting::None;
                count(&mut self.wakes[pid], 1);
                fixed.ipc_wake(pid);
            }
        }
    }
}

pub(super) fn exit(pid: usize, fixed: &mut Fixed) {
    with(|p| {
        p.waits[pid] = Waiting::None;
        p.ipc.exit(pid);
        p.changed(fixed, pid);
    });
}

fn flags(raw: u64) -> Result<u32, i32> {
    u32::try_from(raw).map_err(|_| -22)
}
fn path(address: u64, len: u64, buf: &mut [u8; 31]) -> Result<&[u8], i32> {
    if len == 0 {
        return Err(-2);
    }
    if len > buf.len() as u64 {
        return Err(-36);
    }
    let bytes = &mut buf[..len as usize];
    if !mmu::copy_from_user(address as usize, bytes) {
        return Err(-14);
    }
    Ok(bytes)
}
enum ResultAction {
    Return(usize),
    Block(Waiting),
}

fn transfer(
    p: &mut Pipes,
    pid: usize,
    fd: usize,
    address: usize,
    n: usize,
    write: bool,
) -> Result<ResultAction, i32> {
    let span = if write {
        p.ipc.write_span(pid, fd, n)?
    } else {
        p.ipc.read_span(pid, fd, n)?
    };
    let Some(len) = span else {
        // A future positive transfer requires at least one accessible byte;
        // an atomic write requires the whole record. Recheck the exact span
        // on retry, since another ready owner may have changed the fill level.
        let minimum = if write && n <= PIPE_BUF { n } else { 1 };
        if !mmu::user_range(address, minimum, !write) {
            return Err(-14);
        }
        return Ok(ResultAction::Block(if write {
            Waiting::Write(fd, n)
        } else {
            Waiting::Read(fd, n)
        }));
    };
    if len == 0 {
        return Ok(ResultAction::Return(0));
    }
    if len > CAPACITY {
        fail(9);
    }
    let mut staging = [0u8; CAPACITY];
    let bytes = &mut staging[..len];
    let actual = if write {
        if !mmu::copy_from_user(address, bytes) {
            return Err(-14);
        }
        p.ipc.write(pid, fd, bytes)?
    } else {
        if !mmu::user_range(address, len, true) {
            return Err(-14);
        }
        let actual = p.ipc.read(pid, fd, bytes)?;
        if actual != Some(len) || !mmu::copy_to_user(address, bytes) {
            fail(9);
        }
        actual
    };
    if actual != Some(len) {
        fail(9);
    }
    count(
        if write {
            &mut p.written[pid]
        } else {
            &mut p.read[pid]
        },
        len,
    );
    Ok(ResultAction::Return(len))
}

fn invoke(p: &mut Pipes, pid: usize, f: &mut NativeFrame) -> Result<ResultAction, i32> {
    use ResultAction::{Block, Return};
    let [a, b, c] = [f.x[0], f.x[1], f.x[2]];
    let fd = a as usize;
    let mut name = [0; 31];
    let result = match f.x[8] {
        41 => {
            let (rd, wr) = p.ipc.pipe(pid, flags(a)?)?;
            f.x[1] = rd as u64;
            f.x[2] = wr as u64;
            0
        }
        42 => {
            if b == u64::MAX {
                p.ipc.dup(pid, fd)?
            } else {
                let target = p.ipc.dup2(pid, fd, b as usize)?;
                #[cfg(jnox_native_shell)]
                if target < 3 {
                    p.redirected[pid] |= 1 << target;
                }
                target
            }
        }
        43 => {
            p.ipc.mkfifo(
                pid,
                path(a, b, &mut name)?,
                NATIVE_PIPE_READERS[pid],
                NATIVE_PIPE_WRITERS[pid],
            )?;
            0
        }
        44 => {
            p.ipc.unlink(pid, path(a, b, &mut name)?)?;
            0
        }
        45 => match p.ipc.open(pid, path(a, b, &mut name)?, flags(c)?)? {
            Some(fd) => fd,
            None => return Ok(Block(Waiting::Open)),
        },
        46 | 47 => return transfer(p, pid, fd, b as usize, c as usize, f.x[8] == 47),
        48 => {
            p.ipc.close(pid, fd)?;
            0
        }
        49 => match b {
            1 => p.ipc.getfd(pid, fd)? as usize,
            2 => {
                let arg = flags(c)?;
                if arg & !FD_CLOEXEC != 0 {
                    return Err(-22);
                }
                p.ipc.set_cloexec(pid, fd, arg & FD_CLOEXEC != 0)?;
                0
            }
            3 => p.ipc.getfl(pid, fd)? as usize,
            4 => {
                let arg = flags(c)?;
                if arg & !(O_NONBLOCK | O_WRONLY) != 0 {
                    return Err(-22);
                }
                p.ipc.set_nonblock(pid, fd, arg & O_NONBLOCK != 0)?;
                0
            }
            _ => return Err(-22),
        },
        _ => return Err(-38),
    };
    Ok(Return(result))
}

pub(super) fn service(pid: usize, f: &mut NativeFrame, fixed: &mut Fixed) -> Action {
    if mmu::active() != Some(pid) || fixed.current() != Some(pid) {
        fail(9);
    }
    with(|p| {
        let result = invoke(p, pid, f);
        let action = match result {
            Ok(ResultAction::Return(value)) => {
                f.x[0] = value as u64;
                Action::Poll
            }
            Err(errno) => {
                f.x[0] = i64::from(errno) as u64;
                if f.x[8] == 41 {
                    f.x[1] = 0;
                    f.x[2] = 0;
                }
                Action::Poll
            }
            Ok(ResultAction::Block(w)) => {
                if !matches!(p.waits[pid], Waiting::None) {
                    fail(9);
                }
                if !matches!(w, Waiting::Open) {
                    // Only architectural user registers retain the address.
                    // Resume at the original A64 SVC in the owner's private root.
                    f.elr = f.elr.checked_sub(4).unwrap_or_else(|| fail(9));
                    if !mmu::user_return_pc(f.elr as usize) {
                        fail(9);
                    }
                }
                p.waits[pid] = w;
                count(&mut p.blocks[pid], 1);
                Action::BlockIpc
            }
        };
        p.changed(fixed, pid);
        action
    })
}

fn row(tag: &[u8], fields: &[u64]) {
    super::uart_write(tag);
    for &v in fields {
        super::uart_write(b" ");
        super::unsigned(v);
    }
    super::uart_write(b"\n");
}
pub(super) fn report(phase: u64) {
    with(|p| {
        for i in 0..OBJECTS {
            let s = p.ipc.snapshot(i).unwrap();
            row(
                b"NATIVE PIPE STATE",
                &[
                    phase,
                    i as u64,
                    s.generation,
                    s.allocated as u64,
                    s.linked as u64,
                    s.readers as u64,
                    s.writers as u64,
                    s.fd_refs as u64,
                    s.bytes as u64,
                    s.high_water as u64,
                    p.peaks[i] as u64,
                ],
            );
        }
        for pid in 0..COUNT {
            row(
                b"NATIVE PIPE TASK",
                &[
                    phase,
                    pid as u64,
                    p.blocks[pid],
                    p.wakes[pid],
                    p.read[pid],
                    p.written[pid],
                ],
            );
        }
    })
}

#[cfg(jnox_native_shell)]
pub(super) fn redirect(
    child: usize,
    stdin: Option<usize>,
    stdout: Option<usize>,
) -> Result<(), i32> {
    with(|p| {
        p.ipc.redirect(0, child, stdin, stdout)?;
        if stdin.is_some() {
            p.redirected[child] |= 1;
        }
        if stdout.is_some() {
            p.redirected[child] |= 2;
        }
        Ok(())
    })
}

#[cfg(jnox_native_shell)]
pub(super) fn console_allowed(pid: usize, fd: usize) -> bool {
    matches!(fd, 1 | 2)
        && with(|p| p.redirected[pid] & (1 << fd) == 0 && p.ipc.getfd(pid, fd) == Err(-9))
}
