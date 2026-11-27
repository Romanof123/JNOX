//! Directed observation only. The host supplies an Ethernet frame after WAIT;
//! the guest neither pends an interrupt nor modifies the live virtio transport.
use super::super::{fence, irq, queues, read, STATUS};
use core::arch::asm;

#[unsafe(no_mangle)]
static mut jnox_linux_net_pending_retire: [u64; 24] = [0; 24];
#[unsafe(no_mangle)]
static mut jnox_linux_net_pending_retire_bytes: [u8; 1530] = [0; 1530];

fn put(slot: usize, value: u64) {
    unsafe { (&raw mut jnox_linux_net_pending_retire[slot]).write_volatile(value); }
}

fn phase(value: u64) {
    fence();
    put(0, value);
    fence();
}

fn ticks() -> u64 {
    let n: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

fn invalid() -> bool {
    phase(4);
    false
}

pub(super) fn observe() -> bool {
    let daif: u64;
    let hz: u64;
    unsafe {
        asm!("mrs {}, daif", "mrs {}, cntfrq_el0", out(reg) daif, out(reg) hz,
             options(nostack, preserves_flags));
    }
    let (token, rx) = queues::retirement_snapshot();
    let baseline = rx[1] as u16;
    let old = irq::retirement_state();
    let start = ticks();
    put(1, token);
    put(2, daif);
    put(3, hz);
    put(4, start);
    put(7, u64::from(baseline));
    put(9, old[3]);
    put(10, old[4]);
    // Existing fixture: one successful delivery followed by one unconsumed
    // completion. Accept exactly one further arrival at the masked exit gate.
    if daif & 0x80 == 0 || hz == 0 || hz > u64::MAX / 10 || token == 0 || read(STATUS) != 15
        || rx[19] != token || baseline != 2 || rx[18] != 2 || rx[3] != 1
        || old != [token, 0, 1, 0, 0] || !queues::rx_callbacks_enabled() {
        return invalid();
    }
    let limit = hz * 5;
    put(5, start.wrapping_add(limit));
    phase(1);
    loop {
        let now = ticks();
        if now.wrapping_sub(start) >= limit {
            put(6, now);
            phase(3);
            return false;
        }
        let head = queues::rx_progress().0;
        if head == baseline { continue; }
        let state = irq::retirement_state();
        put(8, u64::from(head));
        put(11, state[3]);
        put(12, state[4]);
        if head != baseline.wrapping_add(1) || state[..3] != [token, 0, 1] {
            return invalid();
        }
        if state[3] == 0 || state[4] & 1 == 0 { continue; }
        let out = unsafe { &mut *(&raw mut jnox_linux_net_pending_retire_bytes) };
        let Some(info) = queues::observe_pending_frame(token, baseline, out) else { return invalid(); };
        for (i, value) in info.into_iter().enumerate() { put(13 + i, value); }
        let observed = ticks();
        put(6, observed);
        put(17, observed.wrapping_add(hz * 2));
        phase(2);
        // Allow QMP to freeze physical DMA before teardown. CNT bounds this
        // observation window even when the host never inspects the machine.
        while ticks().wrapping_sub(observed) < hz * 2 { core::hint::spin_loop(); }
        put(18, ticks());
        let final_state = irq::retirement_state();
        if queues::rx_progress().0 != head || final_state != state { return invalid(); }
        return true;
    }
}
