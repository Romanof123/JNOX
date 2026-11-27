//! Selected single-core, masked EL1 process-signal path. EL0 never supplies a
//! context to restore. One coalesced pending bit per supported signal.
use super::{Action, COUNT, CURRENT, EMPTY_FRAME, EXIT_STATUS, FRAMES, Fixed, NativeFrame,
            STACK_BOTTOM, STACK_TOP, counter, fixed_dispatch, native_el0_mmu as mmu};

const NUMBERS: [u64; 5] = [1, 2, 13, 15, 30]; // Picolibc HUP, INT, PIPE, TERM, USR1
const SUPPORTED: u64 = (1 << 1) | (1 << 2) | (1 << 13) | (1 << 15) | (1 << 30);
const DEFAULT: u64 = 0;
const IGNORE: u64 = 1;

#[derive(Clone, Copy)]
struct Disposition {
    handler: u64,
    mask: u64,
    restorer: u64,
}
const INITIAL: Disposition = Disposition { handler: DEFAULT, mask: 0, restorer: 0 };

#[derive(Clone, Copy)]
struct Saved {
    frame: NativeFrame,
    mask: u64,
    return_pc: u64,
}
const EMPTY_SAVED: Saved = Saved { frame: EMPTY_FRAME, mask: 0, return_pc: 0 };

#[derive(Clone, Copy)]
struct Process {
    actions: [Disposition; NUMBERS.len()],
    saved: [Saved; NUMBERS.len()],
    depth: usize,
    blocked: u64,
    pending: u64,
}
static mut PROCESSES: [Process; COUNT] = [Process {
    actions: [INITIAL; NUMBERS.len()],
    saved: [EMPTY_SAVED; NUMBERS.len()],
    depth: 0, blocked: 0, pending: 0,
}; COUNT];

pub(super) fn retire(pid: usize) {
    // The record contains only integer data. Zero is the initial/default
    // disposition, empty mask and depth; discard saved user registers too.
    unsafe { core::ptr::write_bytes(&raw mut PROCESSES[pid], 0, 1) };
}

pub(super) fn action(pid: usize, number: u64, new: usize, old: usize, restorer: u64) -> i64 {
    if number == 0 || number >= 32 || (new != 0 && matches!(number, 9 | 17)) { return -22; }
    let Some(i) = NUMBERS.iter().position(|&n| number == n) else { return -95 };
    // Direct Picolibc AArch64 layout (statically asserted in the C wrapper).
    let change = if new != 0 {
        let mut wire = [0u8; 24];
        if !mmu::copy_from_user(new, &mut wire) { return -14; }
        let handler = u64::from_le_bytes(wire[..8].try_into().unwrap());
        let mask = u64::from_le_bytes(wire[8..16].try_into().unwrap());
        if wire[16..20] != [0; 4] || mask & !SUPPORTED != 0 { return -95; }
        if handler > IGNORE && (!mmu::signal_code(pid, handler, 4) || !mmu::signal_code(pid, restorer, 12)) {
            return -22;
        }
        Some(Disposition { handler, mask, restorer })
    } else { None };
    let state = unsafe { &mut *(&raw mut PROCESSES[pid]) };
    if old != 0 {
        let mut wire = [0u8; 24];
        wire[..8].copy_from_slice(&state.actions[i].handler.to_le_bytes());
        wire[8..16].copy_from_slice(&state.actions[i].mask.to_le_bytes());
        if !mmu::copy_to_user(old, &wire) { return -14; }
    }
    if let Some(change) = change {
        state.actions[i] = change;
        if change.handler == IGNORE { state.pending &= !(1 << number); }
    }
    0
}

/// Called only after the signal sender passed the SVC 209 process authority check.
pub(super) fn send(fixed: &mut Fixed, target: usize, number: u64) -> i64 {
    let Some(i) = NUMBERS.iter().position(|&n| n == number) else { return -95 };
    if !fixed.has_live_process(target) { return -3; }
    let state = unsafe { &mut *(&raw mut PROCESSES[target]) };
    if state.actions[i].handler == IGNORE { return 0; }
    let bit = 1 << number;
    state.pending |= bit;
    // Only interrupt an actual clock sleep. An unreleased package slot or a
    // blocked broker/device operation retains its existing owner and result.
    if state.blocked & bit == 0 && unsafe { FRAMES[target].x[8] == 7 } && fixed.interrupt_sleep(target) {
        unsafe { FRAMES[target].x[0] = (-4i64) as u64 };
    }
    0
}

pub(super) fn restore(pid: usize, frame: &mut NativeFrame) -> Result<(), i64> {
    let state = unsafe { &mut *(&raw mut PROCESSES[pid]) };
    let top = state.depth.checked_sub(1).ok_or(-1)?;
    let saved = &state.saved[top];
    if frame.elr != saved.return_pc { return Err(-1); }
    *frame = saved.frame;
    state.blocked = saved.mask;
    state.depth = top;
    Ok(())
}

enum Delivery { None, Handler, Terminate(i32) }

fn deliver(pid: usize, frame: &mut NativeFrame) -> Delivery {
    let state = unsafe { &mut *(&raw mut PROCESSES[pid]) };
    let available = state.pending & !state.blocked;
    if available == 0 { return Delivery::None; }
    let number = u64::from(available.trailing_zeros());
    let index = NUMBERS.iter().position(|&n| n == number).expect("supported pending signal");
    let action = state.actions[index];
    state.pending &= !(1 << number);
    if action.handler == DEFAULT { return Delivery::Terminate(128 + number as i32); }
    // Installing IGNORE already discarded all pending instances.
    debug_assert!(action.handler > IGNORE);
    // Every active entry blocks its own signal until return; with five supported
    // numbers no sixth distinct nested callback can be admitted.
    if frame.spsr & 0x1f != 0 || frame.sp_el0 % 16 != 0 ||
        !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&frame.sp_el0) {
        return Delivery::Terminate(139);
    }
    state.saved[state.depth] = Saved { frame: *frame, mask: state.blocked, return_pc: action.restorer + 8 };
    state.depth += 1;
    state.blocked |= action.mask | (1 << number);
    frame.x[0] = number;
    frame.x[30] = action.restorer;
    frame.elr = action.handler;
    Delivery::Handler
}

/// Assembly invokes this with IRQs masked and an EL0 frame, after the normal
/// scheduling decision and before loading any user register for ERET.
#[unsafe(no_mangle)]
extern "C" fn native_signal_return_boundary(frame: &mut NativeFrame) {
    loop {
        let pid = unsafe { CURRENT };
        match deliver(pid, frame) {
            Delivery::None | Delivery::Handler => return,
            Delivery::Terminate(status) => {
                unsafe { EXIT_STATUS[pid] = status };
                fixed_dispatch(counter(), Action::Exit, 210, frame);
            }
        }
    }
}
