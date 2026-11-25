use super::{Frame, HZ, NONE, counter, epoch, print};
use crate::group_policy::{EnergyRequest, OperatingPoint};
use crate::power_policy::{CpuLimits, Decision, place};
use core::sync::atomic::{AtomicU8, AtomicU64, Ordering::*};

include!(concat!(env!("OUT_DIR"), "/native_energy_profile.rs"));

// CPU0 initializes this before the secondary leaves its firmware slot.
// The frame remains immutable for the lifetime of this one-release process.
static mut INITIAL: Frame = Frame::ZERO;
static MASK: AtomicU8 = AtomicU8::new(0);
static FIRST_RETURN: AtomicU64 = AtomicU64::new(NONE);
static FIRST_ENTRY: AtomicU64 = AtomicU64::new(NONE);
static EXIT: AtomicU64 = AtomicU64::new(NONE);

// Only the publishing CPU writes and subsequently prints this record.
#[derive(Clone, Copy)]
struct Record {
    release: u64, start: u64, end: u64, now: u64, deadline: u64,
    disposition: u64, cpu: u64, opp: u64, capacity: u64,
    run: u64, wake: u64, margin: u64, energy_lo: u64, energy_hi: u64,
}
const EMPTY: Record = Record {
    release: NONE, start: NONE, end: NONE, now: NONE, deadline: NONE,
    disposition: 4, cpu: NONE, opp: NONE, capacity: NONE, run: NONE,
    wake: NONE, margin: NONE, energy_lo: NONE, energy_hi: NONE,
};
static mut RECORD: Record = EMPTY;

pub(super) fn initialize(f: Frame) { unsafe { INITIAL = f }; }
pub(super) fn initial_frame() -> Frame { unsafe { INITIAL } }
pub(super) fn owner_mask() -> u8 { MASK.load(Acquire) }

fn point((cpu, opp, capacity, energy_per_work, wake_energy, wake_ticks):
         (u8, u16, u64, u64, u64, u64)) -> OperatingPoint {
    OperatingPoint { cpu, opp, capacity, energy_per_work, wake_energy, wake_ticks }
}

pub(super) fn decide(release: u64) {
    let start = counter();
    let e = epoch();
    let tick = HZ.load(Relaxed) / 10;
    let (work, minimum_capacity, allowed_cpus, deadline, reserve) = ENERGY_REQUEST;
    let mut record = Record { start, release, ..EMPTY };
    let mut mask = 0;
    if let (Some(elapsed), Some(absolute)) = (
        start.checked_sub(e),
        deadline.checked_mul(tick).and_then(|delta| e.checked_add(delta)),
    ) {
        let now = elapsed.div_ceil(tick); // startup requires F >= 50
        record.now = now;
        record.deadline = absolute;
        let request = EnergyRequest { work, minimum_capacity, allowed_cpus, now, deadline };
        let limits = ENERGY_LIMITS.map(|(available, thermal_capacity, shared_load, reserve_capacity)|
            CpuLimits { available, thermal_capacity, shared_load, reserve_capacity });
        let decision = place(request, reserve, &ENERGY_POINTS.map(point), &limits, ENERGY_FALLBACK.map(point));
        record.disposition = match &decision {
            Decision::Idle => 0, Decision::Model(_) => 1,
            Decision::Fallback(_) => 2, Decision::DeadlineMiss => 3,
        };
        if let Decision::Model(p) | Decision::Fallback(p) = decision {
            mask = 1u8 << p.cpu;
            record.cpu = u64::from(p.cpu);
            record.opp = u64::from(p.opp);
            record.capacity = p.capacity;
            record.run = p.run_ticks;
            record.wake = p.wake_ticks;
            record.margin = p.margin;
            record.energy_lo = p.energy as u64;
            record.energy_hi = (p.energy >> 64) as u64;
        }
    }
    record.end = counter();
    unsafe { RECORD = record };
    MASK.store(mask, Release);
}

pub(super) fn observed(returned: u64, entry: u64) {
    // A selected singleton has one fixed CPU and one writer.
    if FIRST_ENTRY.load(Relaxed) == NONE {
        FIRST_RETURN.store(returned, Relaxed);
        FIRST_ENTRY.store(entry, Release);
    }
}
pub(super) fn exit(entry: u64) { EXIT.store(entry, Release); }

pub(super) fn emit() {
    let r = unsafe { RECORD };
    let hz = HZ.load(Relaxed);
    let exit = EXIT.load(Acquire);
    let (miss, magnitude) = if exit == NONE {
        (NONE, NONE)
    } else {
        (u64::from(exit > r.deadline), exit.abs_diff(r.deadline))
    };
    print(b"ENERGY", &[1, hz, hz / 10, epoch(), r.release, r.start, r.end,
        r.now, r.deadline, r.disposition, r.cpu, r.opp, r.capacity, r.run, r.wake,
        r.margin, r.energy_lo, r.energy_hi, FIRST_RETURN.load(Acquire),
        FIRST_ENTRY.load(Acquire), exit, miss, magnitude]);
}
