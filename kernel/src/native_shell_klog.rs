//! Read-only operator diagnostics for the native shell catalog on masked CPU0.
use super::mmu;
use core::fmt::{self, Write};

#[derive(Clone, Copy)]
pub(crate) enum Event {
    NativeBoot,
    ShellReady,
    Spawn(usize),
    Exit(usize, i32),
}

const CAPACITY: usize = 4;
const SNAPSHOT_BYTES: usize = 512;
struct Log {
    rows: [(Event, u64); CAPACITY],
    total: usize,
}
static mut LOG: Log = Log { rows: [(Event::NativeBoot, 0); CAPACITY], total: 0 };

pub(crate) fn push(event: Event, ticks: u64) {
    // CPU0, non-nested EL1. Two one-shot child slots bound total to six.
    let log = unsafe { &mut *(&raw mut LOG) };
    log.rows[log.total % CAPACITY] = (event, ticks);
    log.total += 1;
}

struct Text {
    bytes: [u8; SNAPSHOT_BYTES],
    used: usize,
}
impl Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.used + s.len();
        self.bytes.get_mut(self.used..end).ok_or(fmt::Error)?.copy_from_slice(s.as_bytes());
        self.used = end;
        Ok(())
    }
}

impl Log {
    fn format(&self, text: &mut Text) -> fmt::Result {
        let lost = self.total.saturating_sub(CAPACITY);
        writeln!(text, "dmesg records={} dropped={}", self.total - lost, lost)?;
        for index in lost..self.total {
            let (event, ticks) = self.rows[index % CAPACITY];
            write!(text, "[{}] ticks={} ", index + 1, ticks)?;
            match event {
                Event::NativeBoot => writeln!(text, "native boot")?,
                Event::ShellReady => writeln!(text, "shell ready")?,
                Event::Spawn(pid) => writeln!(text, "child spawn pid={pid}")?,
                Event::Exit(pid, status) => writeln!(text, "child exit pid={pid} status={status}")?,
            }
        }
        Ok(())
    }
}

pub(super) fn snapshot(address: usize, length: usize) -> Result<usize, i32> {
    let mut text = Text { bytes: [0; SNAPSHOT_BYTES], used: 0 };
    // The caller verified slot zero and its active mapping. No append can
    // interleave with this copy on the masked single-core exception path.
    unsafe { (&*(&raw const LOG)).format(&mut text) }.map_err(|_| -75)?;
    if length < text.used { return Err(-28); }
    if !mmu::copy_to_user(address, &text.bytes[..text.used]) { return Err(-14); }
    Ok(text.used)
}
