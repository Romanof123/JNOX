//! Linked timing witnesses. The scheduler sees only the table and RR quantum.
use crate::fixed_runtime::{self as rt, Mode, Task};
use crate::uart_write;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const Q: u64 = 12_500_000;
#[cfg(jnox_app = "fixed-ties")]
const PREFIX: &[u8] = b"FIXED TIES ";
#[cfg(jnox_app = "fixed-late")]
const PREFIX: &[u8] = b"FIXED LATE ";

#[repr(C)]
struct Observations {
    stack: [AtomicU64; 3],
    progress: AtomicU64,
    peer_progress: [AtomicU64; 2],
    started: AtomicU64,
    masked: AtomicU64,
    unmasking: AtomicU64,
    peer_entry: [AtomicU64; 2],
    resumed: AtomicU64,
    phase: AtomicU64,
    failure: AtomicU64,
    reserved: [AtomicU64; 2],
}

#[unsafe(no_mangle)]
static fixed_app_observations: Observations = Observations {
    stack: [const { AtomicU64::new(0) }; 3],
    progress: AtomicU64::new(0),
    peer_progress: [const { AtomicU64::new(0) }; 2],
    started: AtomicU64::new(0),
    masked: AtomicU64::new(0),
    unmasking: AtomicU64::new(0),
    peer_entry: [const { AtomicU64::new(0) }; 2],
    resumed: AtomicU64::new(0),
    phase: AtomicU64::new(0),
    failure: AtomicU64::new(0),
    reserved: [const { AtomicU64::new(0) }; 2],
};

fn marker(suffix: &[u8]) {
    uart_write(PREFIX);
    uart_write(suffix);
}

#[inline(always)]
fn record_stack(role: usize) {
    let sp: u64;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    fixed_app_observations.stack[role].store(sp, Ordering::Relaxed);
}

#[inline(always)]
fn peer(role: usize, suffix: &[u8]) {
    let obs = &fixed_app_observations;
    record_stack(role);
    obs.peer_entry[role - 1].store(rt::counter(), Ordering::Relaxed);
    obs.peer_progress[role - 1].store(obs.progress.load(Ordering::Acquire), Ordering::Relaxed);
    if obs.phase.load(Ordering::Acquire) != role as u64 - 1 {
        obs.failure.store(1, Ordering::Release);
    }
    marker(suffix);
    obs.phase.store(role as u64, Ordering::Release);
}

#[unsafe(no_mangle)]
fn fixed_edge_b() {
    peer(1, b"B RUN\n");
}

#[unsafe(no_mangle)]
fn fixed_edge_c() {
    peer(2, b"C RUN\n");
}

#[unsafe(no_mangle)]
fn fixed_edge_a() {
    // This trusted fixture masks only PSTATE.I. It leaves all platform
    // timer/GIC registers and the other exception masks untouched.
    #[cfg(jnox_app = "fixed-late")]
    unsafe { asm!("msr daifset, #2", "isb", options(nostack, preserves_flags)) };
    let obs = &fixed_app_observations;
    let start = rt::counter();
    obs.started.store(start, Ordering::Relaxed);
    #[cfg(jnox_app = "fixed-late")]
    obs.masked.store(start, Ordering::Relaxed);
    record_stack(0);
    marker(b"A START\n");
    let mut progress = 0;
    #[cfg(jnox_app = "fixed-late")]
    {
        loop {
            progress += 1;
            obs.progress.store(progress, Ordering::Release);
            let now = rt::counter();
            if now.wrapping_sub(start) >= 4 * Q {
                obs.unmasking.store(now, Ordering::Release);
                break;
            }
        }
        unsafe { asm!("msr daifclr, #2", "isb", options(nostack, preserves_flags)) };
    }
    loop {
        if rt::counter().wrapping_sub(start) >= 12 * Q {
            obs.failure.store(2, Ordering::Release);
            marker(b"FAIL\n");
            return;
        }
        if obs.phase.load(Ordering::Acquire) == 2 {
            break;
        }
        progress += 1;
        obs.progress.store(progress, Ordering::Release);
    }
    obs.resumed.store(rt::counter(), Ordering::Release);
    obs.progress.store(progress + 1, Ordering::Release);
    marker(b"A RESUME\n");
    obs.phase.store(3, Ordering::Release);
    marker(b"A DONE\n");
}

// Reverse the two delayed slots so deadline order is visible independently
// of index order. Both fixtures use these logical releases; only A's code
// differs between the two linked images.
static TABLE: [Task; 3] = [
    Task { entry: fixed_edge_c, priority: 2, delay: Q },
    Task { entry: fixed_edge_a, priority: 2, delay: 0 },
    Task { entry: fixed_edge_b, priority: 2, delay: Q / 2 },
];

pub fn run() -> ! {
    core::hint::black_box(&fixed_app_observations);
    marker(b"START\n");
    rt::run(&TABLE, Mode::RoundRobin(Q))
}
