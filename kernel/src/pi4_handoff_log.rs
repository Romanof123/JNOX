//! Frozen MG4 projection. All calls are made under the ownership transaction lock.
//! A diagnostic may treat these words as terminal only after all writers quiesce.
//! A stop within a transaction can expose a prefix, including a partially written
//! installation or payload; it cannot establish a terminal transition.
use crate::pi4_handoff::{FrameInstall, Opening, Origin, Reverse, Slot};
use crate::pi4_migration_log::event;
use core::sync::atomic::{AtomicU64, Ordering};

#[repr(C, align(64))]
struct Terminal {
    header: [u64; 4],
    slots: [[AtomicU64; 9]; 2],
    frames: [[AtomicU64; 3]; 3],
    reserved: u64,
}

const _: () = {
    assert!(core::mem::size_of::<Terminal>() == 256);
    assert!(core::mem::offset_of!(Terminal, slots) == 32);
    assert!(core::mem::offset_of!(Terminal, frames) == 176);
    assert!(core::mem::offset_of!(Terminal, reserved) == 248);
};

#[unsafe(no_mangle)]
static pi4_handoff_terminal_v1: Terminal = Terminal {
    header: [0x3130524554464f48, 1, 256, 0],
    slots: [const { [const { AtomicU64::new(0) }; 9] }; 2],
    frames: [const { [const { AtomicU64::new(0) }; 3] }; 3],
    reserved: 0,
};

fn frame_words(frame: FrameInstall) -> [u64; 3] {
    let origin = match frame.origin { Origin::Fresh => 1, Origin::IrqSaved => 2 };
    [frame.pointer as u64, frame.version, origin]
}

pub(crate) fn installed(task: u8, frame: FrameInstall) {
    for (cell, value) in pi4_handoff_terminal_v1.frames[usize::from(task)].iter().zip(frame_words(frame)) {
        cell.store(value, Ordering::Relaxed);
    }
}

fn transition<T>(cpu: usize, index: usize, slot: Slot<T>, encode: impl FnOnce(T) -> [u64; 8]) {
    let (phase, tag, payload) = match slot {
        Slot::Empty => return,
        Slot::Pending(value) => (1, 16, encode(value)),
        Slot::Consumed(value) => (2, 17, encode(value)),
    };
    let cells = &pi4_handoff_terminal_v1.slots[index];
    for (cell, value) in cells[1..].iter().zip(payload) {
        cell.store(value, Ordering::Relaxed);
    }
    cells[0].store(phase, Ordering::Release);
    event(cpu, tag, payload);
}

pub(crate) fn opening(cpu: usize, slot: Slot<Opening>) {
    transition(cpu, 0, slot, |o| [
        1, o.epoch, u64::from(o.target.task), o.target.sequence, o.retired.sequence, 0, 0, 0,
    ]);
}

pub(crate) fn reverse(cpu: usize, slot: Slot<Reverse>) {
    transition(cpu, 1, slot, |r| {
        let [pointer, version, origin] = frame_words(r.frame);
        [2, r.epoch, u64::from(r.target.task), r.target.sequence, 0, pointer, version, origin]
    });
}
