//! Shared backing descriptors and queued AF_UNIX rights. All access runs on
//! the masked boot CPU. Descriptors, VM mappings and queue entries hold
//! independent references to the same object.
use super::{COUNT, NATIVE_IPC_ROUTES, NATIVE_SOCKET_SERVICE, native_el0_mmu::{self, shared}};
use core::mem::size_of;

const FD_BASE: usize = 96;
const FDS: usize = 8;
const ESCROWS: usize = 8;
// SVC 188 historically attenuates to read-only. Reserve four explicit x1
// selectors; all other x1 values retain that old behavior (x2 ignored).
const DUP_TAG: usize = 0x4a46_4400;

#[derive(Clone, Copy)]
#[repr(C)]
struct Right {
    object_plus_one: u8,
    writable: u8,
}

impl Right {
    const EMPTY: Self = Self { object_plus_one: 0, writable: 0 };

    fn index(self) -> usize {
        usize::from(self.object_plus_one - 1)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Escrow {
    // The domain-wide, never-recycled token of the accepted SEND call.
    id: u64,
    right: Right,
    reserved: [u8; 6],
}

impl Escrow {
    const EMPTY: Self = Self { id: 0, right: Right::EMPTY, reserved: [0; 6] };
}

#[repr(C)]
struct State {
    sizes: [usize; shared::OBJECTS],
    descriptors: [[Right; FDS]; COUNT],
    escrow: [Escrow; ESCROWS],
    // Appended to preserve the #444 observation layout.
    resizable: [bool; shared::OBJECTS],
    seals: [u8; shared::OBJECTS],
    cloexec: [[bool; FDS]; COUNT],
    // A memfd has one open description; dup and SCM_RIGHTS both share it.
    positions: [usize; shared::OBJECTS],
}

const _: () = assert!(size_of::<Right>() == 2 && size_of::<Escrow>() == 16
    && core::mem::offset_of!(State, escrow) == 32 + COUNT * 16);

#[unsafe(no_mangle)]
static mut jnox_native_shared_state: State = State {
    sizes: [0; shared::OBJECTS],
    descriptors: [[Right::EMPTY; FDS]; COUNT],
    escrow: [Escrow::EMPTY; ESCROWS],
    resizable: [false; shared::OBJECTS],
    seals: [0; shared::OBJECTS],
    cloexec: [[false; FDS]; COUNT],
    positions: [0; shared::OBJECTS],
};

fn routed(pid: usize) -> bool {
    NATIVE_IPC_ROUTES[pid] & (1 << NATIVE_SOCKET_SERVICE) != 0
}

impl State {
    fn right(&self, pid: usize, fd: usize) -> Result<Right, i32> {
        let slot = fd.checked_sub(FD_BASE).ok_or(-9)?;
        self.descriptors[pid].get(slot).copied().filter(|r| r.object_plus_one != 0).ok_or(-9)
    }

    fn free_fd(&self, pid: usize) -> Result<usize, i32> {
        self.free_fd_from(pid, 0)
    }

    fn free_fd_from(&self, pid: usize, minimum: usize) -> Result<usize, i32> {
        if minimum >= FD_BASE + FDS { return Err(-22); }
        self.descriptors[pid].iter().enumerate().skip(minimum.saturating_sub(FD_BASE))
            .find(|(_, right)| right.object_plus_one == 0).map(|(slot, _)| slot).ok_or(-24)
    }

    fn create(&mut self, pid: usize, size: usize, resizable: bool) -> Result<usize, i32> {
        if size > shared::MAX_SIZE || (size == 0 && !resizable) { return Err(-22); }
        let slot = self.free_fd(pid)?;
        // The fixed tables themselves are the references. This also preserves
        // a mapping or in-flight right after its last ordinary fd closes.
        let object = (0..shared::OBJECTS).find(|&obj| {
            let id = (obj + 1) as u8;
            !self.descriptors.iter().flatten().any(|fd| fd.object_plus_one == id)
                && !self.escrow.iter().any(|entry| entry.right.object_plus_one == id)
                && !shared::referenced(obj)
        }).ok_or(-28)?;
        shared::clear(object, size);
        self.sizes[object] = size;
        self.resizable[object] = resizable;
        self.seals[object] = 0;
        self.positions[object] = 0;
        self.cloexec[pid][slot] = resizable;
        self.descriptors[pid][slot] = Right { object_plus_one: (object + 1) as u8, writable: 1 };
        Ok(FD_BASE + slot)
    }

    fn escrow_slot(&self, id: u64) -> Option<usize> {
        self.escrow.iter().position(|slot| id != 0 && slot.id == id)
    }

    fn file_io(&mut self, right: Right, command: usize, argument: usize) -> Result<usize, i32> {
        const TRANSFER: usize = 256;
        let object = right.index();
        if !self.resizable[object] { return Err(-95); }
        let size = self.sizes[object];
        let pos = self.positions[object];
        if command <= 2 {
            let base = [0, pos, size][command] as i64;
            let next = base.checked_add(argument as i64).ok_or(-75)?;
            if next < 0 { return Err(-22); }
            self.positions[object] = next as usize;
            return Ok(next as usize);
        }
        let count = command >> 8;
        if count > TRANSFER { return Err(-22); }
        let mut bytes = [0u8; TRANSFER];
        let n = match command & 255 {
            3 => {
                let n = count.min(size.saturating_sub(pos));
                if n == 0 { return Ok(0); }
                shared::read_file(object, pos, &mut bytes[..n]);
                if !native_el0_mmu::copy_to_user(argument, &bytes[..n]) { return Err(-14); }
                n
            }
            4 => {
                if right.writable == 0 { return Err(-9); }
                if count == 0 { return Ok(0); }
                let n = count.min(shared::MAX_SIZE.saturating_sub(pos));
                if n == 0 { return Err(-27); }
                let end = pos + n;
                if end > size {
                    if self.seals[object] & 4 != 0 { return Err(-1); }
                    if shared::referenced(object) { return Err(-16); }
                }
                // Copy first, including when the user buffer aliases this same backing.
                if !native_el0_mmu::copy_from_user(argument, &mut bytes[..n]) { return Err(-14); }
                if end > size {
                    shared::zero_growth(object, size, end);
                    self.sizes[object] = end;
                }
                shared::write_file(object, pos, &bytes[..n]);
                n
            }
            _ => return Err(-22),
        };
        self.positions[object] = pos + n;
        Ok(n)
    }
}

pub(super) fn direct(pid: usize, op: u64, arg: usize, prot: usize, extra: usize) -> Result<usize, i32> {
    if !routed(pid) { return Err(-13); }
    let state = unsafe { &mut *(&raw mut jnox_native_shared_state) };
    match op {
        180 => state.create(pid, arg, false),
        181 | 193 => {
            let (prot, length) = if op == 193 { (prot & 3, Some(prot >> 2)) } else { (prot, None) };
            let right = state.right(pid, arg)?;
            if !matches!(prot, 1 | 3) { return Err(-22); }
            if prot == 3 && right.writable == 0 { return Err(-13); }
            if length.is_some_and(|n| n != state.sizes[right.index()]) { return Err(-95); }
            if state.sizes[right.index()] == 0 { return Err(-22); }
            shared::map(pid, right.index(), state.sizes[right.index()], prot == 3)
        }
        182 => shared::unmap(pid, arg, None).map(|()| 0),
        194 => shared::unmap(pid, arg, Some(prot)).map(|()| 0),
        183 => {
            state.right(pid, arg)?;
            state.descriptors[pid][arg - FD_BASE] = Right::EMPTY;
            Ok(0)
        }
        187 => state.right(pid, arg).map(|right| state.sizes[right.index()]),
        202 => {
            let right = state.right(pid, arg)?;
            state.file_io(right, prot, extra)
        }
        188 => {
            let source = state.right(pid, arg)?;
            let (right, slot, cloexec) = if prot & !3 == DUP_TAG {
                let mode = prot - DUP_TAG;
                if mode > 1 { return Err(-22); }
                (source, state.free_fd_from(pid, extra)?, mode == 1)
            } else {
                (Right { writable: 0, ..source }, state.free_fd(pid)?, false)
            };
            // The masked SVC path publishes both fields as one operation.
            state.cloexec[pid][slot] = cloexec;
            state.descriptors[pid][slot] = right;
            Ok(FD_BASE + slot)
        }
        195 | 196 => {
            state.right(pid, arg)?;
            let flag = &mut state.cloexec[pid][arg - FD_BASE];
            if op == 195 { return Ok(usize::from(*flag)); }
            if prot > 1 { return Err(-22); }
            *flag = prot != 0;
            Ok(0)
        }
        189 if arg == 0 && prot == 0 => state.create(pid, 0, true),
        189 => Err(-22),
        190..=192 => {
            let right = state.right(pid, arg)?;
            let obj = right.index();
            if !state.resizable[obj] { return Err(-95); }
            if op == 192 { return Ok(usize::from(state.seals[obj])); }
            if right.writable == 0 { return Err(-13); }
            if op == 191 {
                // F_SEAL_SEAL, F_SEAL_SHRINK, F_SEAL_GROW.
                if prot & !7 != 0 { return Err(-95); }
                if state.seals[obj] & 1 != 0 { return Err(-1); }
                state.seals[obj] |= prot as u8;
                return Ok(0);
            }
            let old = state.sizes[obj];
            if prot > shared::MAX_SIZE { return Err(-22); }
            if prot == old { return Ok(0); }
            let seal = if prot < old { 2 } else { 4 };
            if state.seals[obj] & seal != 0 { return Err(-1); }
            // This profile has whole-file mappings, with no SIGBUS contract.
            if shared::referenced(obj) { return Err(-16); }
            if prot > old { shared::zero_growth(obj, old, prot); }
            state.sizes[obj] = prot;
            Ok(0)
        }
        _ => unreachable!(),
    }
}

/// Invoked only by the configured broker's authenticated reply callback.
/// Return the caller's reply word and the broker's result respectively.
/// Errors must leave all references and the original reply token intact.
pub(super) fn transfer(caller: usize, op: u64, token: u64, count: u64, arg: u64)
    -> Result<(u64, u64), i32>
{
    if !routed(caller) { return Err(-13); }
    // INSTALL carries CLOEXEC in bit eight of the broker-local count word.
    // Its authenticated client reply still contains only the count and FD.
    let cloexec = op == 185 && count & 0x100 != 0;
    let count = if op == 185 { count & !0x100 } else { count };
    if count > 64 { return Err(-22); }
    let state = unsafe { &mut *(&raw mut jnox_native_shared_state) };
    match op {
        184 => {
            let right = state.right(caller, arg as usize)?;
            let slot = state.escrow.iter().position(|entry| entry.id == 0).ok_or(-28)?;
            state.escrow[slot] = Escrow { id: token, right, reserved: [0; 6] };
            Ok((count, token))
        }
        185 => {
            let escrow_slot = state.escrow_slot(arg).ok_or(-9)?;
            let slot = state.free_fd(caller)?;
            state.cloexec[caller][slot] = cloexec;
            state.descriptors[caller][slot] = state.escrow[escrow_slot].right;
            state.escrow[escrow_slot] = Escrow::EMPTY;
            let fd = (FD_BASE + slot) as u64;
            Ok((count | (fd << 8), fd))
        }
        _ => unreachable!(),
    }
}

pub(super) fn discard(id: u64) -> Result<(), i32> {
    let state = unsafe { &mut *(&raw mut jnox_native_shared_state) };
    let slot = state.escrow_slot(id).ok_or(-116)?;
    state.escrow[slot] = Escrow::EMPTY;
    Ok(())
}

pub(super) fn exit(pid: usize) {
    shared::exit(pid);
    let state = unsafe { &mut *(&raw mut jnox_native_shared_state) };
    state.descriptors[pid].fill(Right::EMPTY);
    // Queued rights survive their sender. The broker owns their disposal on
    // socket teardown. On broker exit the entire queue domain disappears.
    if pid == NATIVE_SOCKET_SERVICE { state.escrow.fill(Escrow::EMPTY); }
}
