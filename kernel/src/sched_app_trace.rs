use core::sync::atomic::{
    AtomicU64,
    Ordering::{Relaxed, Release},
};

const SLOTS: usize = 128;
const DISABLED: u64 = 0;
const ACTIVE: u64 = 1;
const FROZEN: u64 = 2;

const BLOCK: u64 = 1;
const READY: u64 = 2;
const DISPATCH: u64 = 3;
const TIMER_ENTER: u64 = 4;
const APP_RECEIPT: u64 = 5;
const TIMER_EXIT: u64 = 6;

#[repr(C)]
struct Record {
    sequence: AtomicU64,
    body: [AtomicU64; 7],
}

#[repr(C, align(8))]
struct Capture {
    magic: [u8; 8],
    schema: u64,
    header_bytes: u64,
    record_bytes: u64,
    slots: u64,
    capacity: u64,
    state: AtomicU64,
    frequency: AtomicU64,
    boot: u64,
    session: u64,
    committed: AtomicU64,
    kernel_drops: AtomicU64,
    app_drops: AtomicU64,
    flags: AtomicU64,
    cpu: u64,
    reserved: u64,
    records: [Record; SLOTS],
}

const _: () = {
    assert!(core::mem::size_of::<Capture>() == 8320);
    assert!(core::mem::offset_of!(Capture, records) == 128);
    assert!(core::mem::size_of::<Record>() == 64);
};

enum Source {
    Kernel,
    App,
}

impl Capture {
    const fn new(capacity: u64) -> Self {
        Self {
            magic: *b"JNXTRC01",
            schema: 1,
            header_bytes: 128,
            record_bytes: 64,
            slots: SLOTS as u64,
            capacity,
            state: AtomicU64::new(DISABLED),
            frequency: AtomicU64::new(0),
            boot: 1,
            session: 1,
            committed: AtomicU64::new(0),
            kernel_drops: AtomicU64::new(0),
            app_drops: AtomicU64::new(0),
            flags: AtomicU64::new(0),
            cpu: 0,
            reserved: 0,
            records: [const {
                Record {
                    sequence: AtomicU64::new(0),
                    body: [const { AtomicU64::new(0) }; 7],
                }
            }; SLOTS],
        }
    }

    fn start(&self, frequency: u64, enabled: bool) {
        with_irq_masked(|| {
            self.frequency.store(frequency, Relaxed);
            if enabled {
                self.state.store(ACTIVE, Release);
            }
        });
    }

    fn append(&self, source: Source, body: impl FnOnce() -> [u64; 7]) {
        if self.state.load(Relaxed) != ACTIVE {
            return;
        }
        with_irq_masked(|| {
            if self.state.load(Relaxed) != ACTIVE {
                return;
            }
            let index = self.committed.load(Relaxed);
            if index >= self.capacity {
                let drops = match source {
                    Source::Kernel => &self.kernel_drops,
                    Source::App => &self.app_drops,
                };
                let count = drops.load(Relaxed);
                if count == u64::MAX {
                    self.flags.store(1, Relaxed);
                } else {
                    drops.store(count + 1, Relaxed);
                }
                return;
            }
            let record = &self.records[index as usize];
            for (word, value) in record.body.iter().zip(body()) {
                word.store(value, Relaxed);
            }
            record.sequence.store(index + 1, Release);
            self.committed.store(index + 1, Release);
        });
    }

    fn freeze(&self) {
        with_irq_masked(|| {
            if self.state.load(Relaxed) == ACTIVE {
                self.state.store(FROZEN, Release);
            }
        });
    }
}

// Both asm blocks have compiler memory clobbers. Only I changes; D/A/F are untouched.
#[cfg(not(test))]
#[inline(always)]
fn with_irq_masked(f: impl FnOnce()) {
    let prior: u64;
    unsafe {
        core::arch::asm!("mrs {}, daif", "msr daifset, #2", out(reg) prior,
                         options(nostack, preserves_flags));
    }
    f();
    if prior & (1 << 7) == 0 {
        unsafe {
            core::arch::asm!("msr daifclr, #2", options(nostack, preserves_flags));
        }
    }
}

#[used]
#[unsafe(no_mangle)]
static jnox_sched_app_trace_v1: Capture =
    Capture::new(if cfg!(jnox_sched_app_trace_mode = "saturate") {
        4
    } else {
        SLOTS as u64
    });

pub(crate) fn start(freq: u64) {
    jnox_sched_app_trace_v1.start(freq, !cfg!(jnox_sched_app_trace_mode = "disabled"));
}

fn thread(slot: usize) -> u64 {
    (1 << 32) | slot as u64
}

pub(crate) fn block(owner: usize, stop: u64) {
    jnox_sched_app_trace_v1.append(Source::Kernel, || {
        [BLOCK, stop, thread(owner), thread(owner), 0, 0, 0]
    });
}

pub(crate) fn ready(owner: usize, target: usize, w: u64) {
    jnox_sched_app_trace_v1.append(Source::Kernel, || {
        [
            READY,
            super::multilevel_service::counter(),
            thread(owner),
            thread(target),
            w,
            0,
            0,
        ]
    });
}

pub(crate) fn dispatch(out: Option<usize>, next: usize, start: u64) {
    jnox_sched_app_trace_v1.append(Source::Kernel, || {
        [
            DISPATCH,
            start,
            out.map_or(0, thread),
            thread(next),
            0,
            0,
            0,
        ]
    });
}

fn irq(event: u64, owner: usize, line: u32, instance: u64) {
    jnox_sched_app_trace_v1.append(Source::Kernel, || {
        [
            event,
            super::multilevel_service::counter(),
            thread(owner),
            thread(owner),
            line as u64,
            instance,
            0,
        ]
    });
}

pub(crate) fn irq_enter(owner: usize, irq_line: u32, instance: u64) {
    irq(TIMER_ENTER, owner, irq_line, instance);
}

pub(crate) fn receipt(value: u64, end: u64) {
    jnox_sched_app_trace_v1.append(Source::App, || {
        [APP_RECEIPT, end, thread(0), thread(0), value, 0, 0]
    });
}

pub(crate) fn irq_exit(owner: usize, irq_line: u32, instance: u64) {
    irq(TIMER_EXIT, owner, irq_line, instance);
}

pub(crate) fn freeze() {
    jnox_sched_app_trace_v1.freeze();
}

#[cfg(test)]
use tests::with_irq_masked;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    std::thread_local! { static DAIF: Cell<u64> = const { Cell::new(0) }; }
    const I: u64 = 1 << 7;

    // Host model only. Target DAIF instructions require independent disassembly/guest evidence.
    pub(super) fn with_irq_masked(f: impl FnOnce()) {
        DAIF.with(|mask| {
            let prior = mask.get();
            mask.set(prior | I);
            f();
            if prior & I == 0 {
                mask.set(mask.get() & !I);
            }
        });
    }

    #[test]
    fn disabled_and_frozen_skip_body() {
        let capture = Capture::new(4);
        capture.start(100, false);
        capture.append(Source::App, || panic!("disabled writer"));
        capture.freeze();
        assert_eq!(capture.frequency.load(Relaxed), 100);
        assert_eq!(capture.state.load(Relaxed), DISABLED);
        assert_eq!(capture.committed.load(Relaxed), 0);
        assert_eq!(capture.app_drops.load(Relaxed), 0);

        let capture = Capture::new(4);
        capture.start(100, true);
        capture.freeze();
        capture.append(Source::Kernel, || panic!("frozen writer"));
        assert_eq!(capture.state.load(Relaxed), FROZEN);
        assert_eq!(capture.committed.load(Relaxed), 0);
        assert_eq!(capture.kernel_drops.load(Relaxed), 0);
    }

    #[test]
    fn full_capture_preserves_prefix_and_saturates_per_source() {
        let capture = Capture::new(4);
        capture.start(100, true);
        for value in 1..=4 {
            capture.append(Source::Kernel, || [value; 7]);
        }
        capture.kernel_drops.store(u64::MAX - 1, Relaxed);
        capture.append(Source::Kernel, || panic!("full writer"));
        assert_eq!(capture.flags.load(Relaxed), 0);
        capture.append(Source::Kernel, || panic!("overflow writer"));
        capture.append(Source::App, || panic!("app writer"));
        assert_eq!(capture.kernel_drops.load(Relaxed), u64::MAX);
        assert_eq!(capture.app_drops.load(Relaxed), 1);
        assert_eq!(capture.flags.load(Relaxed), 1);
        assert_eq!(capture.committed.load(Relaxed), 4);
        for (index, record) in capture.records.iter().enumerate() {
            let expected = if index < 4 { index as u64 + 1 } else { 0 };
            assert_eq!(record.sequence.load(Relaxed), expected);
            assert!(
                record
                    .body
                    .iter()
                    .all(|word| word.load(Relaxed) == expected)
            );
        }
    }

    #[test]
    fn modeled_irq_stays_pending_and_prior_mask_is_preserved() {
        let capture = Capture::new(4);
        capture.start(100, true);
        let other_bits = (1 << 9) | (1 << 8) | (1 << 6);
        DAIF.with(|mask| {
            mask.set(other_bits);
            let mut pending = false;
            capture.append(Source::App, || {
                assert_eq!(capture.records[0].sequence.load(Relaxed), 0);
                assert_eq!(capture.committed.load(Relaxed), 0);
                pending = mask.get() & I != 0;
                assert!(pending);
                [5; 7]
            });
            assert_eq!(mask.get(), other_bits);
            if pending && mask.get() & I == 0 {
                mask.set(other_bits | I);
                capture.append(Source::Kernel, || [4; 7]);
                assert_eq!(mask.get(), other_bits | I);
            }
            assert_eq!(capture.committed.load(Relaxed), 2);
            assert_eq!(capture.records[0].sequence.load(Relaxed), 1);
            assert_eq!(capture.records[1].sequence.load(Relaxed), 2);
            assert_eq!(capture.records[0].body[0].load(Relaxed), 5);
            assert_eq!(capture.records[1].body[0].load(Relaxed), 4);
        });
    }
}
