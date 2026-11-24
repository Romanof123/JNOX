//! Two coherent QEMU CPUs run a common bounded workload. Energy and OPP
//! parameters are policy inputs. Elapsed times come from the physical counter.
use core::arch::{asm, global_asm};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::group_policy::{EnergyRequest, OperatingPoint};
use super::power_policy::{CpuLimits, Decision, place};
use super::{park, uart_write};

global_asm!(include_str!("power.S"));
unsafe extern "C" {
    fn power_work(out: *mut Work, gate: *const AtomicU64);
}

const ITER: u64 = 4096;
const SUM: u64 = ITER * (ITER + 1) / 2;
const CASES: usize = 6;
const NONE: u64 = u64::MAX;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Work {
    mpidr: u64,
    sp: u64,
    start: u64,
    end: u64,
    result: u64,
    iterations: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Row {
    case: u64,
    decision: u64,
    model_points: u64,
    allowed_mask: u64,
    work: u64,
    min_capacity: u64,
    deadline_ticks: u64,
    deadline_reserve: u64,
    cpu0_thermal: u64,
    cpu0_shared: u64,
    cpu1_thermal: u64,
    selected_cpu: u64,
    opp: u64,
    capacity: u64,
    run_ticks: u64,
    wake_ticks: u64,
    model_margin: u64,
    model_energy: u64,
    release: u64,
    dispatch: u64,
    work_start: u64,
    work_end: u64,
    observed_complete: u64,
    deadline_counter: u64,
    measured_margin: u64,
    mpidr: u64,
    sp: u64,
    result: u64,
    load_start: u64,
    load_end: u64,
    load_batches: u64,
    load_result: u64,
    dispatch_to_work: u64,
    response: u64,
}

#[repr(C, align(64))]
struct Witness {
    header: [u64; 8],
    rows: [Row; CASES],
}
const _: () = assert!(core::mem::size_of::<Row>() == 34 * 8);
#[unsafe(no_mangle)]
static mut power_witness: Witness = unsafe { core::mem::zeroed() };

// Only CPU1 writes REMOTE. A completion release hands it back to CPU0,
// which finishes copying before publishing the next command sequence.
static mut REMOTE: Work = unsafe { core::mem::zeroed() };
static SHARED_GATE: AtomicU64 = AtomicU64::new(0);
static COMMAND: AtomicU64 = AtomicU64::new(0);
static DONE: AtomicU64 = AtomicU64::new(0);
static ONLINE: AtomicU64 = AtomicU64::new(0);

fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

fn require(ok: bool) {
    if !ok {
        uart_write(b"POWER FAIL\n");
        park()
    }
}

fn peer() -> ! {
    ONLINE.store(1, Ordering::Release);
    let mut last = 0;
    loop {
        let command = COMMAND.load(Ordering::Acquire);
        if command == NONE {
            park()
        }
        if command != last {
            let gate = if SHARED_GATE.load(Ordering::Acquire) == 1 {
                &raw const SHARED_GATE
            } else {
                ptr::null()
            };
            unsafe { power_work(&raw mut REMOTE, gate) };
            DONE.store(command, Ordering::Release);
            last = command;
        }
        super::smp_boot::wait_for_peer();
    }
}

fn local_work() -> Work {
    let mut out = Work::default();
    unsafe { power_work(&mut out, ptr::null()) };
    out
}

fn check(work: Work, cpu: u64) {
    require(
        work.mpidr & 0xff00_ffffff == cpu
            && work.sp != 0
            && work.sp & 15 == 0
            && work.result == SUM
            && work.iterations == ITER
            && work.start < work.end,
    );
}

fn background(row: &mut Row) {
    let work = local_work();
    check(work, 0);
    if row.load_batches == 0 {
        row.load_start = work.start;
    }
    row.load_batches += 1;
    row.load_result = work.result;
    row.load_end = work.end;
}

fn execute(row: &mut Row, remote_jobs: &mut u64, frequency: u64) {
    let shared = row.cpu0_shared != 0;
    if shared {
        background(row);
    }
    row.dispatch = counter();
    let work = if row.selected_cpu == 0 {
        local_work()
    } else {
        *remote_jobs += 1;
        SHARED_GATE.store(u64::from(shared), Ordering::Release);
        COMMAND.store(*remote_jobs, Ordering::Release);
        while DONE.load(Ordering::Acquire) != *remote_jobs {
            if shared && SHARED_GATE.load(Ordering::Acquire) == 2 {
                background(row);
                SHARED_GATE.store(3, Ordering::Release);
            }
            require(counter().wrapping_sub(row.dispatch) < frequency * 3);
            super::smp_boot::wait_for_peer();
        }
        unsafe { ptr::read_volatile(&raw const REMOTE) }
    };
    row.observed_complete = counter();
    check(work, row.selected_cpu);
    require(
        row.release <= row.dispatch
            && row.dispatch <= work.start
            && work.end <= row.observed_complete,
    );
    if shared {
        background(row);
        require(
            row.load_start <= work.start
                && row.load_end >= work.end
                && row.load_batches == 3
                && SHARED_GATE.load(Ordering::Acquire) == 3,
        );
    }
    row.mpidr = work.mpidr;
    row.sp = work.sp;
    row.result = work.result;
    row.work_start = work.start;
    row.work_end = work.end;
    row.dispatch_to_work = work.start - row.dispatch;
    row.response = row.observed_complete - row.release;
    row.measured_margin = row.deadline_counter.wrapping_sub(row.observed_complete);
}

fn scenario(case: usize, tick: u64, frequency: u64, remote_jobs: &mut u64) -> Row {
    let points = [
        OperatingPoint {
            cpu: 0,
            opp: 0,
            capacity: 3,
            energy_per_work: 1,
            wake_energy: 1,
            wake_ticks: 1,
        },
        OperatingPoint {
            cpu: 1,
            opp: 1,
            capacity: 6,
            energy_per_work: 3,
            wake_energy: 2,
            wake_ticks: 1,
        },
    ];
    let limit = |thermal_capacity, shared_load| CpuLimits {
        available: true,
        thermal_capacity,
        shared_load,
        reserve_capacity: 0,
    };
    let t0 = match case {
        1 => 2,
        3 => 1,
        _ => 3,
    };
    let t1 = if case == 3 { 1 } else { 6 };
    let shared = u64::from(case == 2);
    let limits = [limit(t0, shared), limit(t1, 0)];
    let mask = if case == 5 { 0 } else { 3 };
    let request = EnergyRequest {
        allowed_cpus: mask,
        work: 12,
        minimum_capacity: 1,
        now: 0,
        deadline: 6,
    };
    let release = counter();
    let mut row = Row {
        case: case as u64,
        model_points: if case == 4 { 0 } else { 2 },
        allowed_mask: mask,
        work: 12,
        min_capacity: 1,
        deadline_ticks: 6,
        deadline_reserve: 1,
        cpu0_thermal: t0,
        cpu0_shared: shared,
        cpu1_thermal: t1,
        selected_cpu: NONE,
        release,
        deadline_counter: release.checked_add(6 * tick).unwrap(),
        ..Row::default()
    };
    let decision = if case == 4 {
        place(request, 1, &[], &limits, Some(points[1]))
    } else {
        place(request, 1, &points, &limits, None)
    };
    let chosen = match decision {
        Decision::Model(p) => {
            row.decision = 1;
            Some(p)
        }
        Decision::Fallback(p) => {
            row.decision = 2;
            Some(p)
        }
        Decision::DeadlineMiss => {
            row.decision = 3;
            None
        }
        Decision::Idle => {
            row.decision = 4;
            None
        }
    };
    if let Some(p) = chosen {
        row.selected_cpu = u64::from(p.cpu);
        row.opp = u64::from(p.opp);
        row.capacity = p.capacity;
        row.run_ticks = p.run_ticks;
        row.wake_ticks = p.wake_ticks;
        row.model_margin = p.margin;
        row.model_energy = u64::try_from(p.energy).unwrap();
    }
    let expected = [(1, 0), (1, 1), (1, 1), (3, NONE), (2, 1), (4, NONE)][case];
    require((row.decision, row.selected_cpu) == expected);
    if row.selected_cpu != NONE {
        execute(&mut row, remote_jobs, frequency);
    }
    row
}

pub(super) fn cpu_main(cpu: usize) -> ! {
    if cpu == 1 {
        peer()
    }
    require(cpu == 0);
    let freq: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nostack, preserves_flags)) };
    require(freq >= 10 && freq <= u32::MAX as u64);
    let start = counter();
    while ONLINE.load(Ordering::Acquire) != 1 {
        require(counter().wrapping_sub(start) < freq * 3);
        super::smp_boot::wait_for_peer();
    }
    let tick = freq / 10;
    let mut remote_jobs = 0;
    let messages: [&[u8]; CASES] = [
        b"POWER BASE\n",
        b"POWER THERMAL\n",
        b"POWER SHARED\n",
        b"POWER DEADLINE MISS\n",
        b"POWER FALLBACK\n",
        b"POWER IDLE\n",
    ];
    // CPU0 alone publishes the journal; completion of the UART trace follows
    // all volatile writes. CPU1 accesses only the mailbox and its own stack.
    unsafe {
        ptr::write_volatile(
            &raw mut power_witness.header,
            [1, 0, 0, freq, tick, ITER, SUM, 0],
        );
    }
    for (case, message) in messages.iter().enumerate() {
        let row = scenario(case, tick, freq, &mut remote_jobs);
        unsafe {
            ptr::write_volatile(&raw mut power_witness.rows[case], row);
            ptr::write_volatile(&raw mut power_witness.header[1], (case + 1) as u64);
        }
        uart_write(message);
    }
    require(remote_jobs == 3);
    COMMAND.store(NONE, Ordering::Release);
    unsafe {
        ptr::write_volatile(&raw mut power_witness.header[7], remote_jobs);
        ptr::write_volatile(&raw mut power_witness.header[2], 1);
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    uart_write(b"POWER PASS\n");
    park()
}
