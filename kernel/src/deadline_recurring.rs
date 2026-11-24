//! Two finite service invocations on the recurring profile's private stack.
use super::*;
use core::sync::atomic::AtomicU64;

pub(super) const OFFSET: u64 = 1;
pub(super) const PERIOD: u64 = 8;
pub(super) const WIDTH: u64 = 1;

// Per invocation: ordinal+1, first input, last input, entry SP, returned sum,
// absolute counter sampled by the caller after its reducer returned.
#[unsafe(no_mangle)]
static deadline_recurring_returns: [[AtomicU64; 6]; 2] =
    [const { [const { AtomicU64::new(0) }; 6] }; 2];

#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn deadline_recurring_reduce(first: u64) -> u64 {
    let mut sum = 0;
    for input in first..first + 64 {
        let value = core::hint::black_box(input);
        sum += value * value;
    }
    sum
}

pub(super) extern "C" fn activate(ordinal: u64) -> ! {
    let record = &deadline_recurring_returns[ordinal as usize];
    let first = 1 + 64 * ordinal;
    let sp: u64;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nostack, preserves_flags)) }
    record[0].store(ordinal + 1, Ordering::Relaxed);
    record[1].store(first, Ordering::Relaxed);
    record[2].store(first + 63, Ordering::Relaxed);
    record[3].store(sp, Ordering::Relaxed);
    let sum = deadline_recurring_reduce(first);
    record[4].store(sum, Ordering::Relaxed);
    record[5].store(counter(), Ordering::Release);
    idle()
}

fn record(ordinal: usize) -> [u64; 6] {
    core::array::from_fn(|i| deadline_recurring_returns[ordinal][i].load(Ordering::Acquire))
}

// Actual calls to initial(), compare() and first sched_start_frame().
#[unsafe(no_mangle)]
pub(super) static deadline_recurring_launch: [AtomicU64; 3] =
    [const { AtomicU64::new(0) }; 3];

pub(super) fn prelaunch() {
    let zero = (0..2).all(|i| record(i) == [0; 6]
        && jobs::PROGRESS[i].load(Ordering::Relaxed) == 0
        && jobs::PRODUCT[i].load(Ordering::Relaxed) == 0);
    let rows = unsafe { ptr::read_volatile(&raw const ordinary_deadline_row_count) };
    line(b"RECURRING_PRELAUNCH", &[i128::from(zero), i128::from(rows)]);
    if !zero || rows != 0 || deadline_recurring_launch.iter().any(|v| v.load(Ordering::Relaxed) != 0) {
        fail()
    }
}

pub(super) fn boundary(rt: &mut Runtime, k: u64, kind: u64, out: usize) {
    if out == SERVICE {
        if kind != 1 || k < OFFSET + WIDTH || (k - OFFSET - WIDTH) % PERIOD != 0 {
            rt.stop()
        }
        let ordinal = ((k - OFFSET - WIDTH) / PERIOD) as usize;
        let [seq, first, last, sp, sum, returned] = record(ordinal);
        if seq != ordinal as u64 + 1 || first != 1 + 64 * ordinal as u64
            || last != first + 63 || sum != [89440, 617824][ordinal]
            || !(top(SERVICE) as u64 - STACK_BYTES as u64..=top(SERVICE) as u64).contains(&sp)
            || !(rt.at(k - WIDTH)..rt.at(k)).contains(&returned) {
            rt.stop()
        }
    }
    if k < HORIZON && k >= OFFSET && (k - OFFSET) % PERIOD == 0 {
        let ordinal = (k - OFFSET) / PERIOD;
        if kind != 1 || out != 0 || record(ordinal as usize) != [0; 6] {
            rt.stop()
        }
        // We are on A's exception stack, and the previous activation has
        // already returned and been taken off the service stack.
        let address = (top(SERVICE) - size_of::<Frame>()) as *mut Frame;
        unsafe { ptr::write_bytes(address, 0, 1) };
        rt.frames[SERVICE] = initial(SERVICE, activate as *const () as usize, ordinal);
    }
}

pub(super) fn finish(rt: &Runtime) {
    let rows = unsafe { &(&*(&raw const ordinary_deadline_rows))[..rt.len] };
    let mut peak_entries = 0;
    let mut peak_changes = 0;
    for cell in 0..=HORIZON {
        let mut entries = 0;
        let mut changes = 0;
        for (i, row) in rows.iter().enumerate() {
            if row.event[1] == cell {
                entries += 1;
                changes += usize::from(i + 1 < rows.len() && row.event[9] != row.event[10]);
            }
        }
        peak_entries = peak_entries.max(entries);
        peak_changes = peak_changes.max(changes);
    }
    line(b"RECURRING_SLOT_PEAK", &[peak_entries, peak_changes as i128, 3]);
    if peak_entries > 3 || peak_changes > 3 { rt.stop() }
    let mut last_progress = 0;
    for ordinal in 0..2 {
        let k = OFFSET + ordinal as u64 * PERIOD;
        let irq = |cell| rows.iter().find(|row| row.event[0] == 1 && row.event[1] == cell);
        let (Some(start), Some(end)) = (irq(k), irq(k + WIDTH)) else { rt.stop() };
        let progress = start.job[0][5];
        if start.event[9..11] != [0, SERVICE as u64] || end.event[9..11] != [SERVICE as u64, 0]
            || start.event[11..13] != end.event[13..15] || progress <= last_progress
            || progress != end.job[0][5] {
            rt.stop()
        }
        line(b"RECURRING_RETURN", &record(ordinal).map(i128::from));
        line(b"RECURRING_PROGRESS", &[k.into(), progress, end.job[0][5]]);
        last_progress = progress;
    }
    for id in 0..2 {
        let blocks = jobs::PROGRESS[id].load(Ordering::Relaxed);
        let product = jobs::PRODUCT[id].load(Ordering::Relaxed);
        line(b"RECURRING_WORK", &[id as i128, blocks.into(), product.into()]);
        if blocks == 0 || (id == 0 && i128::from(blocks) <= last_progress) {
            rt.stop()
        }
    }
}

// A distinct linked PC range for freezing successful and refused guests.
#[inline(never)]
#[unsafe(no_mangle)]
pub(super) extern "C" fn deadline_recurring_terminal() -> ! {
    loop {
        unsafe { asm!("wfi", options(nostack, preserves_flags)) }
    }
}
