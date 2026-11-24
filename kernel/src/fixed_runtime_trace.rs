//! Opt-in physical-memory observer. CPU0 publishes one complete row before
//! incrementing count. Capture readers stop the guest before reading the table.
use super::*;

const WORDS: usize = 19;
const LIMIT: usize = 64;

#[repr(C)]
struct Trace {
    count: u64,
    overflow: u64,
    frequency: u64,
    row_words: u64,
    rows: [[u64; WORDS]; LIMIT],
}

#[unsafe(no_mangle)]
static mut fixed_runtime_trace: Trace = Trace {
    count: 0,
    overflow: 0,
    frequency: 0,
    row_words: 0,
    rows: [[0; WORDS]; LIMIT],
};

pub(super) fn init(hz: u64) {
    unsafe {
        ptr::write_volatile(&raw mut fixed_runtime_trace.frequency, hz);
        ptr::write_volatile(&raw mut fixed_runtime_trace.row_words, WORDS as u64);
    }
}

pub(super) fn ctl() -> u64 {
    let control;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) }
    control
}

pub(super) struct Event {
    pub kind: u64,
    pub source: u64,
    pub now: u64,
    pub owner: u64,
    pub incoming: *mut Frame,
    pub argument: u64,
    pub result: u64,
    pub pre_ctl: u64,
    #[cfg(jnox_pi4_witness)]
    pub pre_cval: u64,
}

fn frame_words(frame: *mut Frame) -> [u64; 3] {
    if frame.is_null() {
        return [0; 3];
    }
    [
        frame.addr() as u64,
        unsafe { (*frame).elr },
        (frame.addr() + size_of::<Frame>()) as u64,
    ]
}

pub(super) fn publish(event: Event, state: &Runtime, next: *mut Frame, due: Option<u64>) {
    // All writers run with IRQs masked. The snapshot object is disjoint from
    // task stacks and policy state. A full table retains its earliest rows.
    let trace = &raw mut fixed_runtime_trace;
    unsafe {
        let count = ptr::read_volatile(&raw const (*trace).count);
        if count >= LIMIT as u64 {
            let dropped = ptr::read_volatile(&raw const (*trace).overflow);
            ptr::write_volatile(&raw mut (*trace).overflow, dropped.saturating_add(1));
            #[cfg(jnox_pi4_witness)]
            witness::invalidate();
            return;
        }
        let [ifp, ipc, isp] = frame_words(event.incoming);
        let [ofp, opc, osp] = frame_words(next);
        let post_ctl = ctl();
        let cval: u64;
        asm!("mrs {}, cntp_cval_el0", out(reg) cval, options(nostack, preserves_flags));
        let service = state
            .policy
            .service(event.owner as usize)
            .unwrap_or(u64::MAX);
        let row = [
            event.kind,
            event.source,
            event.now,
            event.owner,
            state.policy.current().unwrap_or(IDLE) as u64,
            ifp,
            ipc,
            isp,
            ofp,
            opc,
            osp,
            event.argument,
            event.result,
            u64::from(due.is_some()),
            due.unwrap_or(0),
            post_ctl,
            cval,
            event.pre_ctl,
            service,
        ];
        ptr::write_volatile(&raw mut (*trace).rows[count as usize], row);
        asm!("dmb sy", options(nostack, preserves_flags));
        ptr::write_volatile(&raw mut (*trace).count, count + 1);
        #[cfg(jnox_pi4_witness)]
        witness::selected(count as usize, event.pre_cval);
    }
}

#[cfg(jnox_pi4_witness)]
pub(super) fn status() -> (u64,u64) {
    unsafe { (fixed_runtime_trace.count, fixed_runtime_trace.overflow) }
}
#[cfg(jnox_pi4_witness)]
pub(super) fn row(i:usize) -> [u64;19] {
    unsafe { fixed_runtime_trace.rows[i] }
}
