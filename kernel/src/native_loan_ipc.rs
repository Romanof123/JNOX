//! Nonblocking CPU-only page loans. Called only from the serialized native fixed
//! trap; the policy invokes a synchronous mapping callback at its ownership boundary.
use super::{COUNT, NATIVE_LOAN_ROUTE, NativeFrame, fail, native_el0_mmu as mmu, uart_write, unsigned};

#[path = "loan_ipc.rs"]
mod channel;
use channel::{LoanIpc, MapOp};

static mut CHANNEL: LoanIpc = LoanIpc::new(NATIVE_LOAN_ROUTE[0], NATIVE_LOAN_ROUTE[1]);

// Retain actual leaf descriptor transitions. Work remains independent of UART
// speed until terminal output. A full buffer does not change the channel.
const LIMIT: usize = 128;
const WORDS: usize = 4 + 2 * COUNT;
static mut TRANSITIONS: [[u64; WORDS]; LIMIT] = [[0; WORDS]; LIMIT];
static mut STORED: usize = 0;
static mut DROPPED: u64 = 0;

fn leaves(slot: usize) -> [u64; COUNT] {
    core::array::from_fn(|pid| mmu::loan_pte(pid, slot).unwrap_or_else(|| fail(9)))
}

fn apply(op: MapOp) -> Result<(), i32> {
    let (kind, pid, slot, len) = match op {
        MapOp::ZeroMapWriter { pid, slot } => (1, pid, slot, 0),
        MapOp::RevokeWriterClearTail { pid, slot, len } => (2, pid, slot, len),
        MapOp::MapReader { pid, slot } => (3, pid, slot, 0),
        MapOp::Revoke { pid, slot } => (4, pid, slot, 0),
    };
    let before = leaves(slot);
    // A mapping error after a partial table transition cannot safely be
    // returned to EL0 as a retryable syscall.
    match op {
        MapOp::ZeroMapWriter { .. } => mmu::loan_map_writer(pid, slot),
        MapOp::RevokeWriterClearTail { .. } => mmu::loan_publish(pid, slot, len),
        MapOp::MapReader { .. } => mmu::loan_map_reader(pid, slot),
        MapOp::Revoke { .. } => mmu::loan_revoke(pid, slot),
    }.unwrap_or_else(|_| fail(9));
    let after = leaves(slot);
    unsafe {
        if STORED < LIMIT {
            let mut row = [0; WORDS];
            row[..4].copy_from_slice(&[kind, pid as u64, slot as u64, len as u64]);
            row[4..4 + COUNT].copy_from_slice(&before);
            row[4 + COUNT..].copy_from_slice(&after);
            TRANSITIONS[STORED] = row;
            STORED += 1;
        } else {
            DROPPED = DROPPED.saturating_add(1);
        }
    }
    Ok(())
}

#[cfg(jnox_native_safety_channel)]
pub(super) fn invalidate() {
    unsafe { (&mut *(&raw mut CHANNEL)).invalidate(apply) }.unwrap_or_else(|_| fail(9));
}

// Keep the service boundary visible to the host phase probe.
#[inline(never)]
pub(super) fn service(pid: usize, f: &mut NativeFrame) {
    let channel = unsafe { &mut *(&raw mut CHANNEL) };
    let op = f.x[8];
    #[cfg(jnox_native_safety_channel)]
    if let Err(errno) = super::safety::before_loan(pid, op) {
        f.x[..4].fill(0);
        f.x[0] = i64::from(errno) as u64;
        return;
    }
    #[cfg(jnox_native_safety_channel)]
    let handle = f.x[0];
    let result = if op == 35 {
        match channel.receive(pid, apply) {
            Ok(g) => {
                f.x[1] = g.handle;
                f.x[2] = mmu::loan_va(g.slot) as u64;
                f.x[3] = g.len as u64;
                Ok(())
            }
            Err(errno) => {
                f.x[1..4].fill(0);
                Err(errno)
            }
        }
    } else {
        service_other(channel, pid, op, f)
    };
    #[cfg(jnox_native_safety_channel)]
    super::safety::after_loan(pid, op, handle, f, result, channel.occupancy());
    f.x[0] = i64::from(result.err().unwrap_or(0)) as u64;
}

// Keep publish snapshot storage and the other operations off the receive path.
#[inline(never)]
fn service_other(channel: &mut LoanIpc, pid: usize, op: u64, f: &mut NativeFrame) -> Result<(), i32> {
    match op {
        33 => {
            let grant = channel.acquire(pid, apply);
            f.x[1] = 0;
            f.x[2] = 0;
            grant.map(|g| {
                f.x[1] = g.handle;
                f.x[2] = mmu::loan_va(g.slot) as u64;
            })
        }
        34 => {
            #[cfg(jnox_native_safety_channel)]
            let snapshot = {
                let slot = (f.x[0] & 3) as usize;
                let mut fields = [0; 24];
                (f.x[1] == 44 && channel.page(slot).is_some_and(|p| p.state == 1 && p.handle == f.x[0])
                    && mmu::copy_from_user(mmu::loan_va(slot) + 8, &mut fields)).then_some(fields)
            };
            #[cfg(jnox_native_safety_channel)]
            let before = super::counter();
            let status = channel.publish(pid, f.x[0], f.x[1], apply);
            #[cfg(jnox_native_safety_channel)]
            super::safety::published(f.x[0], snapshot, before, super::counter(), status.err().unwrap_or(0));
            status
        }
        36 => channel.release(pid, f.x[0], apply),
        _ => unreachable!(),
    }
}

pub(super) fn exit(pid: usize) {
    if NATIVE_LOAN_ROUTE.contains(&pid) {
        unsafe { (&mut *(&raw mut CHANNEL)).exit(pid, apply) }.unwrap_or_else(|_| fail(9));
    }
}

fn print(tag: &[u8], row: &[u64]) {
    uart_write(tag);
    for &value in row {
        uart_write(b" ");
        unsigned(value);
    }
    uart_write(b"\n");
}

pub(super) fn report() {
    let channel = unsafe { &*(&raw const CHANNEL) };
    let stored = unsafe { STORED };
    for index in 0..stored {
        print(b"NATIVE LOAN MAP", unsafe { &*(&raw const TRANSITIONS[index]) });
    }
    print(b"NATIVE LOAN STATE", &[channel.head(), channel.tail(), channel.occupancy() as u64,
        channel.high_water() as u64, u64::from(channel.producer_alive()), u64::from(channel.consumer_alive()),
        stored as u64, unsafe { DROPPED }]);
    for slot in 0..3 {
        let page = channel.page(slot).unwrap_or_else(|| fail(9));
        print(b"NATIVE LOAN PAGE", &[slot as u64, u64::from(page.state), page.handle, page.len as u64]);
    }
}
