//! One-shot private process launches by shell slot zero. Single masked CPU;
//! no user slice survives a syscall and no child is eligible until commit.
#[path = "native_shell_klog.rs"]
pub(super) mod klog;

use super::{
    Action, COUNT, EMPTY_FRAME, FRAMES, Fixed, NATIVE_EXEC_NAMES, NativeFrame, STACK_BOTTOM,
    STACK_TOP, counter, fail, native_el0_mmu as mmu, native_stack, pipes,
};

const _: () = assert!(COUNT == 3);
const ARG_LIMIT: usize = 16;
const STRING_LIMIT: usize = 1024;
const NO_FD: u64 = u64::MAX;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Dormant,
    Live,
    Zombie,
    Reaped,
    Dismissed,
}
#[derive(Clone, Copy)]
struct Child {
    phase: Phase,
    status: i32,
    spawned: u64,
    exited: u64,
    waited: u64,
    argc: usize,
    stdin: u64,
    stdout: u64,
    entry: u64,
    sp: u64,
    orphan: bool,
}
impl Child {
    const EMPTY: Self = Self {
        phase: Phase::Dormant,
        status: 0,
        spawned: 0,
        exited: 0,
        waited: 0,
        argc: 0,
        stdin: NO_FD,
        stdout: NO_FD,
        entry: 0,
        sp: 0,
        orphan: false,
    };
}
static mut CHILD: [Child; COUNT] = [Child::EMPTY; COUNT];
static mut WAIT: Option<usize> = None;
const CHUNK: usize = 256;
const LOG_ROWS: usize = 32;
#[derive(Clone, Copy)]
struct Console {
    pid: usize,
    fd: usize,
    n: usize,
    bytes: [u8; CHUNK],
}
static mut LOG: [Console; LOG_ROWS] = [Console {
    pid: 0,
    fd: 0,
    n: 0,
    bytes: [0; CHUNK],
}; LOG_ROWS];
static mut USED: usize = 0;
static mut DROPPED: u64 = 0;

fn cstring(address: usize, buf: &mut [u8]) -> Result<usize, i32> {
    for (i, b) in buf.iter_mut().enumerate() {
        if !mmu::copy_from_user(address.checked_add(i).ok_or(-14)?, core::slice::from_mut(b)) {
            return Err(-14);
        }
        if *b == 0 {
            return Ok(i);
        }
    }
    Err(-7)
}
fn descriptor(raw: u64) -> Result<Option<usize>, i32> {
    if raw == NO_FD {
        Ok(None)
    } else if raw <= 10 {
        Ok(Some(raw as usize))
    } else {
        Err(-9)
    }
}

fn launch(f: &NativeFrame, fixed: &mut Fixed) -> Result<usize, i32> {
    let mut program = [0; STRING_LIMIT];
    let name_len = cstring(f.x[0] as usize, &mut program)?;
    let child = (1..COUNT)
        .find(|&i| NATIVE_EXEC_NAMES[i].as_bytes() == &program[..name_len])
        .ok_or(-2)?;
    if !fixed.dormant(child) || unsafe { CHILD[child].phase } != Phase::Dormant {
        return Err(-16);
    }
    let stdin = descriptor(f.x[2])?;
    let stdout = descriptor(f.x[3])?;
    let vector = f.x[1] as usize;
    let mut strings = [0; STRING_LIMIT];
    let mut parts = [(0, 0); ARG_LIMIT];
    let mut end = 0;
    let mut argc = 0;
    loop {
        let mut ptr = [0; 8];
        if !mmu::copy_from_user(vector.checked_add(argc * 8).ok_or(-14)?, &mut ptr) {
            return Err(-14);
        }
        let arg = u64::from_le_bytes(ptr) as usize;
        if arg == 0 {
            break;
        }
        if argc == ARG_LIMIT {
            return Err(-7);
        }
        let len = cstring(arg, &mut strings[end..])?;
        if core::str::from_utf8(&strings[end..end + len]).is_err() {
            return Err(-22);
        }
        parts[argc] = (end, len);
        end += len + 1;
        argc += 1;
    }
    if argc == 0 || parts[0].1 == 0 {
        return Err(-22);
    }
    let argv: [&[u8]; ARG_LIMIT] = core::array::from_fn(|i| {
        let (start, len) = parts[i];
        &strings[start..start + len]
    });
    let mut tail = [0; native_stack::MAX_TAIL];
    let layout =
        native_stack::build_argv(&argv[..argc], STACK_BOTTOM, STACK_TOP, &mut tail).ok_or(-7)?;
    let bytes = &tail[..layout.len];
    mmu::validate_dormant_stack(child, layout.sp, bytes).map_err(|_| -14)?;
    // The pipe transaction preflights both directions and destination slots.
    // No fallible user action follows it: MMU preconditions cannot change
    // until this masked handler completes.
    pipes::redirect(child, stdin, stdout)?;
    mmu::install_dormant_stack(child, layout.sp, bytes).unwrap_or_else(|_| fail(9));
    unsafe {
        let entry = FRAMES[child].elr;
        FRAMES[child] = NativeFrame {
            elr: entry,
            sp_el0: layout.sp as u64,
            ..EMPTY_FRAME
        };
        core::ptr::write_volatile(
            &raw mut super::jnox_native_startup_slots[child],
            [entry, layout.sp as u64, layout.len as u64, argc as u64],
        );
        CHILD[child] = Child {
            phase: Phase::Live,
            spawned: counter(),
            argc,
            stdin: f.x[2],
            stdout: f.x[3],
            entry,
            sp: layout.sp as u64,
            ..Child::EMPTY
        };
    }
    fixed.launch(child);
    klog::push(klog::Event::Spawn(child), unsafe { CHILD[child].spawned });
    Ok(child)
}

fn wait(child: usize, f: &mut NativeFrame) -> Result<Action, i32> {
    if !(1..COUNT).contains(&child) {
        return Err(-10);
    }
    unsafe {
        match CHILD[child].phase {
            Phase::Live => {
                WAIT = Some(child);
                Ok(Action::BlockIpc)
            }
            Phase::Zombie => {
                f.x[1] = i64::from(CHILD[child].status) as u64;
                CHILD[child].phase = Phase::Reaped;
                CHILD[child].waited = counter();
                Ok(Action::Poll)
            }
            _ => Err(-10),
        }
    }
}

pub(super) fn exit(pid: usize, status: i32, fixed: &mut Fixed) {
    if pid == 0 {
        unsafe {
            WAIT = None;
        }
        for id in 1..COUNT {
            let phase = unsafe { CHILD[id].phase };
            match phase {
                Phase::Dormant => {
                    // Release any separately configured boot descriptors too.
                    pipes::exit(id, fixed);
                    fixed.dismiss(id);
                    unsafe {
                        CHILD[id].phase = Phase::Dismissed;
                    }
                }
                Phase::Live => unsafe {
                    CHILD[id].orphan = true;
                },
                Phase::Zombie => unsafe {
                    CHILD[id].orphan = true;
                    CHILD[id].phase = Phase::Reaped;
                },
                _ => {}
            }
        }
        return;
    }
    unsafe {
        let phase = CHILD[pid].phase;
        if phase != Phase::Live {
            fail(9);
        }
        CHILD[pid].exited = counter();
        CHILD[pid].status = status;
        klog::push(klog::Event::Exit(pid, status), CHILD[pid].exited);
        let waiting = WAIT;
        if waiting == Some(pid) {
            WAIT = None;
            FRAMES[0].x[0] = 0;
            FRAMES[0].x[1] = i64::from(status) as u64;
            CHILD[pid].waited = counter();
            CHILD[pid].phase = Phase::Reaped;
            fixed.ipc_wake(0);
        } else {
            CHILD[pid].phase = if CHILD[pid].orphan {
                Phase::Reaped
            } else {
                Phase::Zombie
            };
        }
    }
}
fn console(pid: usize, fd: usize, address: usize, len: usize) -> Result<usize, i32> {
    if !pipes::console_allowed(pid, fd) {
        return Err(-9);
    }
    let n = len.min(CHUNK);
    let mut bytes = [0; CHUNK];
    if !mmu::copy_from_user(address, &mut bytes[..n]) {
        return Err(-14);
    }
    super::uart_write(&bytes[..n]);
    unsafe {
        if USED < LOG_ROWS {
            LOG[USED] = Console { pid, fd, n, bytes };
            USED += 1;
        } else {
            DROPPED = DROPPED.saturating_add(1);
        }
    }
    Ok(n)
}
pub(super) fn service(pid: usize, f: &mut NativeFrame, fixed: &mut Fixed) -> Action {
    if fixed.current() != Some(pid) || mmu::active() != Some(pid) {
        fail(9);
    }
    let op = f.x[8];
    let arg = f.x[0];
    let result = match op {
        4 => {
            console(pid, arg as usize, f.x[1] as usize, f.x[2] as usize).map(|n| (n, Action::Poll))
        }
        _ if pid != 0 => Err(-13),
        70 => klog::snapshot(arg as usize, f.x[1] as usize).map(|n| (n, Action::Poll)),
        71 => crate::uart_try_read()
            .map(|byte| (usize::from(byte), Action::Poll))
            .map_err(|errno| errno as i32),
        50 => launch(f, fixed).map(|id| (id, Action::Poll)),
        51 => {
            f.x[1] = 0;
            wait(arg as usize, f).map(|action| (0, action))
        }
        _ => Err(-38),
    };
    match result {
        Ok((value, action)) => {
            f.x[0] = value as u64;
            action
        }
        Err(errno) => {
            f.x[0] = i64::from(errno) as u64;
            Action::Poll
        }
    }
}
pub(super) fn started(pid: usize) -> bool {
    matches!(
        unsafe { CHILD[pid].phase },
        Phase::Live | Phase::Zombie | Phase::Reaped
    )
}
fn print(tag: &[u8], values: &[u64]) {
    super::uart_write(tag);
    for &v in values {
        super::uart_write(b" ");
        super::unsigned(v);
    }
    super::uart_write(b"\n");
}
pub(super) fn report() {
    for pid in 1..COUNT {
        let c = unsafe { CHILD[pid] };
        let phase = match c.phase {
            Phase::Dormant => 0,
            Phase::Live => 1,
            Phase::Zombie => 2,
            Phase::Reaped => 3,
            Phase::Dismissed => 4,
        };
        print(
            b"NATIVE SHELL CHILD",
            &[
                pid as u64,
                phase,
                c.spawned,
                c.exited,
                c.waited,
                i64::from(c.status) as u64,
                c.argc as u64,
                c.stdin,
                c.stdout,
                c.entry,
                c.sp,
                c.orphan as u64,
            ],
        );
    }
    for i in 0..unsafe { USED } {
        let line = unsafe { LOG[i] };
        super::uart_write(b"NATIVE SHELL CONSOLE");
        for n in [i, line.pid, line.fd, line.n] {
            super::uart_write(b" ");
            super::unsigned(n as u64);
        }
        for &byte in &line.bytes[..line.n] {
            super::uart_write(b" ");
            super::unsigned(byte as u64);
        }
        super::uart_write(b"\n");
    }
    print(b"NATIVE SHELL CONSOLE DROPPED", &[unsafe { DROPPED }]);
}
