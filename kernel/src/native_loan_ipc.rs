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

pub(super) fn service(pid: usize, f: &mut NativeFrame) {
    let channel = unsafe { &mut *(&raw mut CHANNEL) };
    let op = f.x[8];
    let result = match op {
        33 | 35 => {
            let grant = if op == 33 { channel.acquire(pid, apply) } else { channel.receive(pid, apply) };
            f.x[1] = 0;
            f.x[2] = 0;
            if op == 35 { f.x[3] = 0; }
            grant.map(|g| {
                f.x[1] = g.handle;
                f.x[2] = mmu::loan_va(g.slot) as u64;
                if op == 35 { f.x[3] = g.len as u64; }
            })
        }
        34 => channel.publish(pid, f.x[0], f.x[1], apply),
        36 => channel.release(pid, f.x[0], apply),
        _ => unreachable!(),
    };
    f.x[0] = i64::from(result.err().unwrap_or(0)) as u64;
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
