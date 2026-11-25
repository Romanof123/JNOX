//! Deterministic component exercise in an ordinary, single-process JNOX app.
//! Its three role IDs and counter operands are private fixture inputs. The
//! linked services use the same policy, register, ring and journals as EL1.
#![no_std]
#![no_main]

use core::ptr::write_volatile;
use core::sync::atomic::{AtomicU64, Ordering};
use jnox_safety_channel::{FRAME_BYTES, Permit, decode, encode, validate_loan};

mod evidence;
use evidence::{uart_write, unsigned};
#[path = "../../../kernel/src/native_loan_ipc.rs"]
mod loans;
mod native_el0_mmu;
#[path = "../../../kernel/src/native_safety_channel.rs"]
mod safety;

const SENDER: usize = 0;
const CONTROLLER: usize = 1;
const SUPERVISOR: usize = 2;
const COUNT: usize = 3;
const NATIVE_LOAN_ROUTE: [usize; 2] = [SENDER, CONTROLLER];
const HZ: u64 = 62_500_000;
const MS: u64 = HZ / 1000;
const EPOCH: u64 = 1_000_000;
const ISSUE3: u64 = EPOCH + 20 * MS;
const ISSUE4: u64 = ISSUE3 + 19 * MS;
const DEADLINE: u64 = ISSUE3 + 20 * MS;

static mut CLOCK_DOMAIN: (u64, u64) = (0, HZ);
static mut CURRENT: usize = SUPERVISOR;
static OPERAND: AtomicU64 = AtomicU64::new(0);

// Only x0..x30 of the native trap frame are read by these component services.
struct NativeFrame {
    x: [u64; 31],
}

fn counter() -> u64 {
    OPERAND.load(Ordering::Relaxed)
}

fn advance(tick: u64) {
    if tick < OPERAND.swap(tick, Ordering::Relaxed) {
        fail(12);
    }
}

fn fail(code: u64) -> ! {
    let status = 100 + code as i32;
    unsafe {
        write_volatile(&raw mut evidence::permit_queue_header[11], status as u64);
    }
    jnox_native::exit(status)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    fail(19)
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
enum Case {
    Before = 1,
    Exact,
    After,
}

impl Case {
    fn tick(self) -> u64 {
        match self {
            Self::Before => DEADLINE - 1,
            Self::Exact => DEADLINE,
            Self::After => DEADLINE + 1,
        }
    }
}

type Result<T> = core::result::Result<T, i32>;

fn require(condition: bool, code: i32) -> Result<()> {
    if condition { Ok(()) } else { Err(code) }
}

fn call(pid: usize, op: u64, args: [u64; 5]) -> NativeFrame {
    let mut frame = NativeFrame { x: [0; 31] };
    frame.x[..5].copy_from_slice(&args);
    frame.x[8] = op;
    unsafe {
        CURRENT = pid;
    }
    match op {
        33..=36 => loans::service(pid, &mut frame),
        59..=61 => safety::service(pid, &mut frame),
        _ => fail(11),
    }
    frame
}

fn control(cmd: u64) -> i64 {
    call(CONTROLLER, 60, [cmd, 0, 0, 0, 0]).x[0] as i64
}

fn supervise(cmd: u64) -> i64 {
    call(SUPERVISOR, 59, [cmd, 1, 0, 0, 0]).x[0] as i64
}

fn commit(frame: Permit) -> i64 {
    call(
        CONTROLLER,
        60,
        [
            2,
            frame.generation,
            frame.sequence,
            frame.issue,
            u64::from(frame.permit),
        ],
    )
    .x[0] as i64
}

fn info(stage: usize, flags: u64, seq: u64, issue: u64) -> Result<()> {
    let service = call(SUPERVISOR, 61, [0; 5]);
    let row: [u64; 7] = service.x[..7].try_into().unwrap();
    unsafe {
        write_volatile(&raw mut evidence::permit_queue_info[stage], row);
    }
    require(row == [0, counter(), HZ, 1, flags, seq, issue], 91)
}

fn permit(sequence: u64, issue: u64) -> Permit {
    Permit {
        generation: 1,
        sequence,
        issue,
        permit: true,
    }
}

fn publish(frame: Permit) -> Result<(u64, [u8; FRAME_BYTES])> {
    let bytes = encode(frame).map_err(|_| 82)?;
    let grant = call(SENDER, 33, [0; 5]);
    require(grant.x[0] == 0 && grant.x[1] != 0, 83)?;
    require(native_el0_mmu::write_grant(grant.x[2] as usize, &bytes), 83)?;
    let handle = grant.x[1];
    let sent = call(SENDER, 34, [handle, FRAME_BYTES as u64, 0, 0, 0]);
    require(sent.x[0] == 0, 84)?;
    Ok((handle, bytes))
}

fn receive(handle: u64, expected: &[u8; FRAME_BYTES], candidate: bool) -> Result<Permit> {
    let received = call(CONTROLLER, 35, [0; 5]);
    if candidate {
        record_attempt(0, received.x[0] as i64);
    }
    require(received.x[0] == 0 && received.x[1] == handle, 85)?;
    validate_loan(
        SENDER,
        CONTROLLER,
        (handle & 3) as usize,
        received.x[3] as usize,
    )
    .map_err(|_| 85)?;
    let mut private = [0; FRAME_BYTES];
    require(
        native_el0_mmu::copy_from_user(received.x[2] as usize, &mut private),
        85,
    )?;
    let released = call(CONTROLLER, 36, [handle, 0, 0, 0, 0]);
    if candidate {
        record_attempt(1, released.x[0] as i64);
        unsafe {
            write_volatile(&raw mut evidence::permit_queue_frames[1], private);
        }
    }
    require(released.x[0] == 0 && private == *expected, 86)?;
    decode(&private).map_err(|_| 86)
}

fn record_attempt(index: usize, status: i64) {
    unsafe {
        write_volatile(&raw mut evidence::permit_queue_attempts[index], status);
    }
}

fn setup_permit(seq: u64, issue: u64) -> Result<()> {
    advance(issue);
    let frame = permit(seq, issue);
    let (handle, bytes) = publish(frame)?;
    advance(issue + 1);
    require(control(1) == 0, 87)?;
    let private = receive(handle, &bytes, false)?;
    require(
        private == frame && commit(private) == 0 && control(3) == 0,
        87,
    )
}

fn run(case: Case) -> Result<()> {
    let tick = case.tick();
    let positive = case == Case::Before;
    let final_seq = if positive { 4 } else { 3 };
    let final_issue = if positive { ISSUE4 } else { ISSUE3 };
    let flags = if positive { 3 } else { 5 }; // Gate open and ENABLE, or gate open and FAULT.
    unsafe {
        write_volatile(
            &raw mut evidence::permit_queue_header,
            [
                0x51435031,
                1,
                case as u64,
                HZ,
                EPOCH,
                ISSUE3,
                ISSUE4,
                DEADLINE,
                ISSUE4 + 1,
                tick,
                0,
                0,
            ],
        );
    }
    advance(EPOCH - 1);
    safety::boot();
    require(supervise(1) == 0, 81)?;
    setup_permit(1, EPOCH)?;
    setup_permit(2, EPOCH + 10 * MS)?;
    advance(EPOCH + 15 * MS);
    require(
        supervise(2) == 0 && control(1) == 0 && control(4) == 0 && control(3) == 0,
        88,
    )?;
    setup_permit(3, ISSUE3)?;
    info(0, 3, 3, ISSUE3)?;

    advance(ISSUE4 + 1);
    let frame = permit(4, ISSUE4);
    let (handle, bytes) = publish(frame)?;
    unsafe {
        write_volatile(&raw mut evidence::permit_queue_header[10], handle);
        write_volatile(&raw mut evidence::permit_queue_frames[0], bytes);
    }
    info(1, 3, 3, ISSUE3)?;
    advance(tick);
    require(evidence::ring(0, Some(handle), true), 89)?;
    require(control(1) == 0, 89)?;
    info(2, flags, 3, ISSUE3)?;
    require(evidence::ring(1, Some(handle), true), 89)?;

    let private = receive(handle, &bytes, true)?;
    require(private == frame && evidence::ring(2, None, true), 89)?;
    let status = commit(private);
    record_attempt(2, status);
    require(status == if positive { 0 } else { -11 }, 90)?;
    info(3, flags, final_seq, final_issue)?;
    for index in 3..=4 {
        let empty = call(CONTROLLER, 35, [0; 5]);
        record_attempt(index, empty.x[0] as i64);
        require(empty.x[0] as i64 == -11, 92)?;
    }
    require(control(3) == 0, 92)?;

    advance(DEADLINE + 2);
    let rearm = supervise(2);
    record_attempt(5, rearm);
    require(rearm == -11 && control(1) == 0, 93)?;
    let consume = control(4);
    record_attempt(6, consume);
    require(consume == -11 && control(3) == 0, 93)?;
    info(4, flags, final_seq, final_issue)?;

    advance(DEADLINE + 3);
    for pid in [CONTROLLER, SENDER, SUPERVISOR] {
        unsafe {
            CURRENT = pid;
        }
        safety::exit(pid);
        loans::exit(pid);
    }
    info(5, 4, final_seq, final_issue)?;
    require(evidence::ring(3, None, false), 94)
}

fn parse_case(argc: i32, argv: *const *const u8) -> Option<Case> {
    if argc != 2 || argv.is_null() {
        return None;
    }
    let arg = unsafe { *argv.add(1) };
    if arg.is_null() {
        return None;
    }
    let len = (0..7).find(|&i| unsafe { *arg.add(i) == 0 })?;
    match unsafe { core::slice::from_raw_parts(arg, len) } {
        b"before" => Some(Case::Before),
        b"exact" => Some(Case::Exact),
        b"after" => Some(Case::After),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let Some(case) = parse_case(argc, argv) else {
        return 80;
    };
    let result = run(case);
    safety::report();
    let status = result
        .and_then(|()| {
            let (seq, issue, streak) = if case == Case::Before {
                (4, ISSUE4, 1)
            } else {
                (3, ISSUE3, 2)
            };
            require(
                evidence::decision_and_identity(case.tick(), ISSUE3, seq, issue, streak),
                95,
            )
        })
        .err()
        .unwrap_or(71);
    unsafe {
        write_volatile(&raw mut evidence::permit_queue_header[11], status as u64);
    }
    status
}
