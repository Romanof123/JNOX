//! Terminal UART journal for the two online CPUs in the Pi migration case.
//! The workload seals its local journal after recording its quiescent event.
use core::arch::asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const CAPACITY: usize = 96;
const QUIESCENT: u64 = 10;
const WORDS: usize = 8;

struct Row {
    kind: AtomicU64,
    words: [AtomicU64; WORDS],
}

impl Row {
    const fn new() -> Self {
        Self {
            kind: AtomicU64::new(0),
            words: [const { AtomicU64::new(0) }; WORDS],
        }
    }
}

struct Journal {
    rows: [Row; CAPACITY],
    committed: AtomicUsize,
    lost: AtomicUsize,
    frequency: AtomicU64,
    sealed: AtomicBool,
}

impl Journal {
    const fn new() -> Self {
        Self {
            rows: [const { Row::new() }; CAPACITY],
            committed: AtomicUsize::new(0),
            lost: AtomicUsize::new(0),
            frequency: AtomicU64::new(0),
            sealed: AtomicBool::new(false),
        }
    }
}

static LOG: [Journal; 2] = [const { Journal::new() }; 2];
static INVALID: AtomicBool = AtomicBool::new(false);

// Disable IRQ before reading CPU identity and reserving a row. A worker can
// otherwise be interrupted and resumed on another CPU between these operations.
// The asm blocks include compiler memory barriers around the entire short write.
struct LocalWriter {
    saved_daif: u64,
    cpu: usize,
}

impl LocalWriter {
    fn enter() -> Self {
        let saved_daif: u64;
        let mpidr: u64;
        unsafe {
            asm!(
                "mrs {saved}, daif",
                "msr daifset, #2",
                "mrs {mpidr}, mpidr_el1",
                saved = out(reg) saved_daif,
                mpidr = out(reg) mpidr,
                options(nostack, preserves_flags)
            );
        }
        Self {
            saved_daif,
            cpu: (mpidr & 0xff) as usize,
        }
    }
}

impl Drop for LocalWriter {
    fn drop(&mut self) {
        unsafe {
            asm!("msr daif, {}", in(reg) self.saved_daif, options(nostack, preserves_flags));
        }
    }
}

/// Append one local observation. Sequence numbers are zero-based and never wrap.
/// Index i becomes visible only after its tag and all eight words are complete.
pub(crate) fn event(cpu: usize, tag: u64, data: [u64; WORDS]) {
    let local = LocalWriter::enter();
    if cpu != local.cpu || cpu >= LOG.len() {
        INVALID.store(true, Ordering::Release);
        return;
    }
    let journal = &LOG[cpu];
    if journal.sealed.load(Ordering::Acquire) {
        INVALID.store(true, Ordering::Release);
        return;
    }
    let index = journal.committed.load(Ordering::Relaxed);
    let Some(row) = journal.rows.get(index) else {
        journal.lost.fetch_add(1, Ordering::Relaxed);
        return;
    };
    row.kind.store(tag, Ordering::Relaxed);
    for (slot, value) in row.words.iter().zip(data) {
        slot.store(value, Ordering::Relaxed);
    }
    journal.committed.store(index + 1, Ordering::Release);
}

/// Called by each CPU on its quiescent path, before acknowledging the peer.
/// Seal even an overflowed journal so its retained prefix can be exported as a failure.
pub(crate) fn seal(cpu: usize) -> bool {
    let local = LocalWriter::enter();
    if cpu != local.cpu || cpu >= LOG.len() {
        INVALID.store(true, Ordering::Release);
        return false;
    }
    let journal = &LOG[cpu];
    if journal.sealed.load(Ordering::Acquire) {
        INVALID.store(true, Ordering::Release);
        return false;
    }
    let used = journal.committed.load(Ordering::Acquire);
    let terminal = used.checked_sub(1).and_then(|i| journal.rows.get(i));
    let quiescent = terminal.is_some_and(|row| {
        row.kind.load(Ordering::Relaxed) == QUIESCENT
            && row.words[0].load(Ordering::Relaxed) == cpu as u64
    });
    if !quiescent {
        INVALID.store(true, Ordering::Release);
    }
    let frequency: u64;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags));
    }
    journal.frequency.store(frequency, Ordering::Relaxed);
    journal.sealed.store(true, Ordering::Release);
    true
}

struct Serial;

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::uart_write(s.as_bytes());
        Ok(())
    }
}

/// CPU0 calls this only after its local seal and the peer's quiescent acknowledgment.
pub(crate) fn export(case: u64, frequency: u64, totals: [u64; 3]) -> ! {
    let local = LocalWriter::enter();
    if local.cpu != 0 {
        crate::park();
    }
    if !LOG
        .iter()
        .all(|journal| journal.sealed.load(Ordering::Acquire))
    {
        crate::uart_write(b"MG FAIL UNSEALED\n");
        crate::park();
    }
    let mut uart = Serial;
    let mut valid = case <= 2 && frequency != 0 && !INVALID.load(Ordering::Acquire);
    let mut total_rows = 0;
    let _ = writeln!(uart, "MG 1 BEGIN {case} 3");
    for (cpu, journal) in LOG.iter().enumerate() {
        let used = journal.committed.load(Ordering::Acquire);
        let lost = journal.lost.load(Ordering::Relaxed);
        let hz = journal.frequency.load(Ordering::Relaxed);
        valid &= lost == 0 && hz == frequency;
        total_rows += used;
        let _ = writeln!(uart, "MG CPU {cpu} {used} {lost} {hz} 1");
        for (seq, row) in journal.rows.iter().take(used).enumerate() {
            let kind = row.kind.load(Ordering::Relaxed);
            let _ = write!(uart, "MG ROW {cpu} {seq} {kind}");
            for word in &row.words {
                let _ = write!(uart, " {}", word.load(Ordering::Relaxed));
            }
            let _ = writeln!(uart);
        }
    }
    let _ = writeln!(
        uart,
        "MG END {} {total_rows} {} {} {}",
        valid as u8, totals[0], totals[1], totals[2]
    );
    crate::park()
}
