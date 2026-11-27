//! Shared backing descriptors and queued AF_UNIX rights. All access runs on
//! the masked boot CPU. Descriptors, VM mappings and queue entries hold
//! independent references to the same object.
use super::{COUNT, NATIVE_IPC_ROUTES, NATIVE_SOCKET_SERVICE, native_el0_mmu::shared};
use core::mem::size_of;

const FD_BASE: usize = 96;
const FDS: usize = 8;
const ESCROWS: usize = 8;

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
        self.descriptors[pid].iter().position(|r| r.object_plus_one == 0).ok_or(-24)
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
        self.cloexec[pid][slot] = resizable;
        self.descriptors[pid][slot] = Right { object_plus_one: (object + 1) as u8, writable: 1 };
        Ok(FD_BASE + slot)
    }

    fn escrow_slot(&self, id: u64) -> Option<usize> {
        self.escrow.iter().position(|slot| id != 0 && slot.id == id)
    }
}

pub(super) fn direct(pid: usize, op: u64, arg: usize, prot: usize) -> Result<usize, i32> {
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
        188 => {
            let right = state.right(pid, arg)?;
            let slot = state.free_fd(pid)?;
            state.cloexec[pid][slot] = false;
            state.descriptors[pid][slot] = Right { writable: 0, ..right };
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
