//! Two-CPU dispatch and migration on coherent QEMU virt memory.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::context::*;
use super::smp_policy::{Disposition, Lease, Ownership};
use super::{park, uart_write};

const M: usize = 0;
const P0: usize = 1;
const P1: usize = 2;
const PARTITION: usize = 0;
const GLOBAL: usize = 1;
const CLUSTER: usize = 2;
const CONTENDED: u64 = 1 << 3;
const P1_FINISHED: u64 = 1 << 4;
const MIGRATED: u64 = 1 << 5;
const IRQ_BIT: u32 = 6;

static LOCK: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static ONLINE: AtomicU64 = AtomicU64::new(0);
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static MPIDR: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
static GATES: AtomicU64 = AtomicU64::new(0);
static mut STACKS: [[Stack; 3]; 3] = [const { [const { Stack([0; STACK_BYTES]) }; 3] }; 3];
static mut RUNTIME: Option<Runtime> = None;

unsafe extern "C" {
    static sched_vectors: u8;
    static smp_handler_tops: [usize; 2];
    fn smp_task_entry();
    static smp_m_work_begin: u8;
    static smp_m_work_end: u8;
    static smp_cluster_resume: u8;
    static smp_after_finish: u8;
}

global_asm!(
    ".set JNOX_SMP, 1",
    include_str!("context.S"),
    include_str!("smp.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    stack_size = const STACK_BYTES,
    gates = sym GATES,
    irq_bit = const IRQ_BIT,
    enter = sym task_enter,
    cluster_resume = sym cluster_resume,
    migrated = sym migrated,
    svc = sym service_call,
    irq = sym timer_interrupt,
    fail = sym fail,
);

// Flat little-endian u64 ABI. Rows are committed under the scheduler lock.
// Only CPU-owned counter intervals are charged, once per successful retire.
#[repr(C)]
struct Audit {
    header: [u64; 5],        // version, completed epochs, interval count, MPIDR0, MPIDR1
    attempts: [[u64; 7]; 2], // epoch, attempting CPU, task, ready, owner+1, CPU idle, rejected
    migration: [u64; 12],    // src, dst, saved ELR, selected ELR, saved SP, live SP, X19,
    // saved X20, live X20, saved TLS, SIMD low, SIMD high
    charged: [[u64; 3]; 3], // epoch, task
    owners: [[u64; 3]; 3],  // bitset of CPUs actually charged per epoch/task
    rows: [[u64; 8]; 11],   // epoch, task, CPU, lease sequence, begin, end, units, MPIDR
    timing: [u64; 3],       // CNTFRQ, CPU0 IRQ handler entry, CPU1 resumed task callback
}

#[unsafe(export_name = "smp_audit")]
static mut AUDIT: Audit = Audit {
    header: [0; 5],
    attempts: [[0; 7]; 2],
    migration: [0; 12],
    charged: [[0; 3]; 3],
    owners: [[0; 3]; 3],
    rows: [[0; 8]; 11],
    timing: [0; 3],
};

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    if !FAILED.swap(true, Ordering::AcqRel) {
        uart_write(b"SMP FAIL\n");
    }
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn mpidr() -> u64 {
    let value: u64;
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn cpu_id() -> usize {
    let affinity = mpidr() & 0xff00_ffffff;
    require(affinity < 2);
    affinity as usize
}

fn counter() -> u64 {
    let value: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

// Both bootstrap callers, task callbacks and exception handlers have IRQs
// masked. No operation can wait for its peer while holding this lock.
fn locked<R>(f: impl FnOnce() -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    while LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        while LOCK.load(Ordering::Relaxed) {
            super::smp_boot::wait_for_peer();
        }
    }
    let result = f();
    LOCK.store(false, Ordering::Release);
    result
}

fn with_runtime<R>(f: impl FnOnce(&mut Runtime, &mut Audit) -> R) -> R {
    // SAFETY: All mutable scheduler and audit accesses use this exclusive lock.
    locked(|| unsafe {
        f(
            (*(&raw mut RUNTIME)).as_mut().unwrap_or_else(|| fail()),
            &mut *(&raw mut AUDIT),
        )
    })
}

fn wait_bits(value: &AtomicU64, bits: u64) {
    while value.load(Ordering::Acquire) & bits != bits {
        super::smp_boot::wait_for_peer();
    }
}

fn handler_stack(cpu: usize) {
    let sp: usize;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nostack, preserves_flags)) };
    let top = unsafe { smp_handler_tops[cpu] };
    require((top - STACK_BYTES..=top).contains(&sp) && sp & 15 == 0);
}

struct Runtime {
    epoch: usize,
    ownership: Ownership<3>,
    ready: u8,
    entered: u8,
    finished: u8,
    leases: [Option<Lease>; 2],
    starts: [u64; 2],
    frames: [*mut Frame; 3],
    saved_pc: u64,
    rejected: bool,
    resumed: bool,
    terminal: bool,
}

impl Runtime {
    fn new(epoch: usize) -> Self {
        let allowed = if epoch == GLOBAL {
            [3, 1, 2]
        } else {
            [1, 1, 2]
        };
        GATES.store(0, Ordering::Release);
        let mut s = Self {
            epoch,
            ownership: Ownership::new(allowed).unwrap_or_else(|| fail()),
            ready: 0,
            entered: 0,
            finished: 0,
            leases: [None; 2],
            starts: [0; 2],
            frames: [ptr::null_mut(); 3],
            saved_pc: 0,
            rejected: false,
            resumed: false,
            terminal: false,
        };
        for id in 0..3 {
            // A unique stack per epoch and task keeps old exception frames
            // disjoint, including while another CPU begins the next epoch.
            let frame = unsafe { s.stack_top(id).sub(size_of::<Frame>()).cast::<Frame>() };
            unsafe {
                (*frame).elr = smp_task_entry as *const () as usize as u64;
                (*frame).spsr = 0x3c5;
                (*frame).gpr[19] = s.witness(id);
                (*frame).gpr[21] = id as u64;
                (*frame).gpr[22] = epoch as u64;
            }
            s.frames[id] = frame;
        }
        s.release((1 << M) | (1 << P1));
        s
    }

    fn stack_top(&self, id: usize) -> *mut u8 {
        require(id < 3);
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 3 + id + 1) * STACK_BYTES)
    }

    fn witness(&self, id: usize) -> u64 {
        0x5100 + 16 * self.epoch as u64 + id as u64
    }

    fn release(&mut self, mask: u8) {
        require(self.ownership.release(u32::from(mask)));
        self.ready |= mask;
    }

    fn check_frame(&self, cpu: usize, frame: *mut Frame) -> usize {
        let lease = self.leases[cpu].unwrap_or_else(|| fail());
        let id = usize::from(lease.task);
        require(usize::from(lease.cpu) == cpu && self.frames[id].is_null());
        require(frame.addr() == self.stack_top(id).addr() - 16 - size_of::<Frame>());
        let witness = self.witness(id);
        let saved = unsafe { &*frame };
        let word = unsafe { ptr::read_volatile(self.stack_top(id).sub(16).cast::<u64>()) };
        require(saved.spsr & 0xf == 5 && saved.gpr[19] == witness && word == witness);
        require(saved.gpr[21] == id as u64 && saved.gpr[22] == self.epoch as u64);
        id
    }

    fn retire(&mut self, cpu: usize, end: u64, disposition: Disposition, audit: &mut Audit) {
        let lease = self.leases[cpu].take().unwrap_or_else(|| fail());
        let id = usize::from(lease.task);
        let begin = self.starts[cpu];
        let units = end.checked_sub(begin).unwrap_or_else(|| fail());
        require(units > 0 && self.ownership.retire(lease, units, disposition));
        // A duplicated retirement must never charge the interval again.
        require(!self.ownership.retire(lease, units, Disposition::Dormant));
        let n = audit.header[2] as usize;
        require(n < audit.rows.len());
        audit.rows[n] = [
            self.epoch as u64,
            id as u64,
            cpu as u64,
            lease.sequence as u64,
            begin,
            end,
            units,
            mpidr(),
        ];
        audit.header[2] += 1;
        let total = &mut audit.charged[self.epoch][id];
        *total = total.checked_add(units).unwrap_or_else(|| fail());
        audit.owners[self.epoch][id] |= 1 << cpu;
        require(self.ownership.charged(lease.task) == Some(*total));
    }

    fn try_dispatch(&mut self, cpu: usize, audit: &mut Audit) -> Option<*mut Frame> {
        require(self.leases[cpu].is_none());
        if self.terminal {
            return None;
        }
        if self.epoch == GLOBAL && cpu == 1 && !self.rejected && self.finished & (1 << P1) != 0 {
            // CPU1 is idle; CPU0 has a live M lease and is waiting for this attempt.
            require(self.leases[0].is_some_and(|lease| lease.task == M as u8));
            require(self.ready & (1 << M) == 0 && self.ownership.claim(1, M as u8).is_none());
            audit.attempts[0] = [GLOBAL as u64, 1, M as u64, 0, 1, 1, 1];
            self.rejected = true;
            uart_write(b"SMP GLOBAL CONTENDED\n");
            GATES.fetch_or(CONTENDED, Ordering::Release);
        }
        if self.epoch == CLUSTER && self.saved_pc != 0 && !self.rejected {
            if cpu == 0 {
                return None;
            }
            // Both CPUs are now idle and M is ready. Only cluster membership
            // prevents CPU1 taking it. Selection on CPU0 starts after this probe.
            require(self.leases[0].is_none() && self.ready & (1 << M) != 0);
            require(self.ownership.claim(1, M as u8).is_none());
            audit.attempts[1] = [CLUSTER as u64, 1, M as u64, 1, 0, 1, 1];
            self.rejected = true;
            uart_write(b"SMP CLUSTER REJECTED\n");
        }
        let mut ranked = [0u8; 3];
        let mut n = 0;
        for id in [P0, M, P1] {
            if self.ready & (1 << id) != 0 {
                ranked[n] = id as u8;
                n += 1;
            }
        }
        let plan = self
            .ownership
            .plan(3, &ranked[..n])
            .unwrap_or_else(|| fail());
        let id = usize::from(plan[cpu]?);
        let lease = self
            .ownership
            .claim(cpu as u8, id as u8)
            .unwrap_or_else(|| fail());
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        if id == M && self.saved_pc != 0 {
            let elr = unsafe { (*frame).elr };
            require(
                elr == self.saved_pc
                    && frame.addr() == self.stack_top(id).addr() - 16 - size_of::<Frame>(),
            );
            if self.epoch == GLOBAL {
                require(cpu == 1);
                audit.migration[3] = elr;
            } else {
                require(cpu == 0);
            }
        }
        self.ready &= !(1 << id);
        self.leases[cpu] = Some(lease);
        self.starts[cpu] = counter();
        Some(frame)
    }

    fn finish_epoch(&mut self, audit: &mut Audit) {
        require(self.entered == 7 && self.ready == 0 && self.leases == [None; 2]);
        let m_cpus = if self.epoch == GLOBAL { 3 } else { 1 };
        require(audit.owners[self.epoch] == [m_cpus, 1, 2]);
        require(self.epoch == PARTITION || (self.resumed && self.rejected));
        audit.header[1] += 1;
        match self.epoch {
            PARTITION => {
                uart_write(b"SMP PARTITION PASS\n");
                *self = Runtime::new(GLOBAL);
            }
            GLOBAL => {
                *self = Runtime::new(CLUSTER);
            }
            CLUSTER => {
                require(audit.header[2] == 11);
                self.terminal = true;
                uart_write(b"SMP CLUSTER PASS\nSMP PASS\n");
            }
            _ => fail(),
        }
    }
}

fn dispatch(cpu: usize) -> *mut Frame {
    loop {
        let (frame, terminal) = with_runtime(|s, audit| (s.try_dispatch(cpu, audit), s.terminal));
        if let Some(frame) = frame {
            return frame;
        }
        if terminal {
            park();
        }
        super::smp_boot::wait_for_peer();
    }
}

pub(super) fn cpu_main(cpu: usize) -> ! {
    require(cpu_id() == cpu);
    if cpu == 0 {
        // Only CPU0 changes the distributor or enables its banked CNTP PPI.
        init_timer_irq();
    } else {
        unsafe {
            asm!("msr vbar_el1, {}", "msr cntp_ctl_el0, xzr", "isb",
                in(reg) &raw const sched_vectors, options(nostack, preserves_flags))
        };
    }
    MPIDR[cpu].store(mpidr(), Ordering::Relaxed);
    ONLINE.fetch_or(1 << cpu, Ordering::AcqRel);
    wait_bits(&ONLINE, 3);
    if cpu == 0 {
        locked(|| unsafe {
            let frequency: u64;
            asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags));
            AUDIT.timing[0] = frequency;
            AUDIT.header = [
                1,
                0,
                0,
                MPIDR[0].load(Ordering::Relaxed),
                MPIDR[1].load(Ordering::Relaxed),
            ];
            RUNTIME = Some(Runtime::new(PARTITION));
            uart_write(b"SMP BOTH ONLINE\n");
        });
        INITIALIZED.store(true, Ordering::Release);
    } else {
        while !INITIALIZED.load(Ordering::Acquire) {
            super::smp_boot::wait_for_peer();
        }
    }
    unsafe { sched_start_frame(dispatch(cpu)) }
}

extern "C" fn task_enter(id: usize, sp: usize) {
    let cpu = cpu_id();
    let epoch = with_runtime(|s, _| {
        require(s.leases[cpu].is_some_and(|lease| usize::from(lease.task) == id));
        require(cpu == usize::from(id == P1) && sp == s.stack_top(id).addr() - 16);
        require(s.entered & (1 << id) == 0);
        s.entered |= 1 << id;
        GATES.fetch_or(1 << id, Ordering::Release);
        s.epoch
    });
    if id != P0 {
        wait_bits(&GATES, (1 << M) | (1 << P1));
    }
    if id == M && epoch == GLOBAL {
        wait_bits(&GATES, CONTENDED);
    }
    if id == M && epoch == CLUSTER {
        wait_bits(&GATES, P1_FINISHED);
    }
    // Both initial leases were dispatched concurrently. In GLOBAL the replacement
    // pinned job also stays active until M executes on the destination CPU.
    if id == P0 && epoch == GLOBAL {
        wait_bits(&GATES, MIGRATED);
    }
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let end = counter();
    let cpu = cpu_id();
    require(cpu == 0);
    handler_stack(cpu);
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_runtime(|s, audit| {
        require(
            s.epoch == GLOBAL && s.saved_pc == 0 && s.rejected && s.check_frame(cpu, frame) == M,
        );
        let saved = unsafe { &*frame };
        let range = (&raw const smp_m_work_begin).addr()..(&raw const smp_m_work_end).addr();
        let witness = s.witness(M);
        let count = saved.gpr[20];
        require(range.contains(&(saved.elr as usize)) && saved.spsr & 0x80 == 0);
        require(count >> 32 == witness && count as u32 >= 128);
        require(saved.tpidr_el1 == witness && saved.simd[31] == [witness; 2]);
        unsafe { ptr::write_volatile(s.stack_top(M).sub(8).cast::<u64>(), count) };
        s.saved_pc = saved.elr;
        audit.timing[1] = end;
        audit.migration = [
            0,
            0,
            saved.elr,
            0,
            (s.stack_top(M).addr() - 16) as u64,
            0,
            witness,
            count,
            0,
            saved.tpidr_el1,
            saved.simd[31][0],
            saved.simd[31][1],
        ];
        // Already on CPU0's exception stack. Frame publication, source lease
        // retirement and admission of P0 share one release/acquire critical section.
        s.frames[M] = frame;
        s.retire(cpu, end, Disposition::Ready, audit);
        s.ready |= 1 << M;
        s.release(1 << P0);
        uart_write(b"SMP GLOBAL IRQ\n");
        GATES.fetch_or(1 << IRQ_BIT, Ordering::Release);
    });
    dispatch(cpu)
}

extern "C" fn migrated(sp: usize, witness: u64, count: u64) {
    let observed = counter();
    let cpu = cpu_id();
    with_runtime(|s, audit| {
        require(s.epoch == GLOBAL && !s.resumed && cpu == 1 && s.saved_pc != 0);
        require(s.leases[cpu].is_some_and(|lease| lease.task == M as u8));
        require(sp == s.stack_top(M).addr() - 16 && witness == s.witness(M));
        require(audit.migration[2] == audit.migration[3]);
        require(
            count
                .checked_sub(audit.migration[7])
                .is_some_and(|delta| delta <= 1),
        );
        audit.migration[1] = cpu as u64;
        audit.migration[5] = sp as u64;
        audit.migration[8] = count;
        require(observed >= audit.timing[1]);
        audit.timing[2] = observed;
        s.resumed = true;
        uart_write(b"SMP GLOBAL MIGRATED\n");
        GATES.fetch_or(MIGRATED, Ordering::Release);
    });
}

extern "C" fn cluster_resume(sp: usize) {
    let cpu = cpu_id();
    with_runtime(|s, _| {
        require(cpu == 0 && s.epoch == CLUSTER && s.rejected && !s.resumed);
        require(s.leases[cpu].is_some_and(|lease| lease.task == M as u8));
        require(
            sp == s.stack_top(M).addr() - 16
                && s.saved_pc == (&raw const smp_cluster_resume).addr() as u64,
        );
        s.resumed = true;
    });
}

extern "C" fn service_call(frame: *mut Frame) -> *mut Frame {
    let end = counter();
    let cpu = cpu_id();
    handler_stack(cpu);
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15);
    let service = esr & 0x01ff_ffff;
    with_runtime(|s, audit| {
        let id = s.check_frame(cpu, frame);
        let pc = unsafe { (*frame).elr };
        if service == 2 {
            require(s.epoch == CLUSTER && id == M && cpu == 0 && s.saved_pc == 0);
            require(pc == (&raw const smp_cluster_resume).addr() as u64);
            s.saved_pc = pc;
            s.frames[id] = frame;
            s.retire(cpu, end, Disposition::Ready, audit);
            s.ready |= 1 << id;
            s.release(1 << P0);
            return;
        }
        require(service == 1 && pc == (&raw const smp_after_finish).addr() as u64);
        require(id != M || s.epoch == PARTITION || s.resumed);
        s.retire(cpu, end, Disposition::Dormant, audit);
        require(s.finished & (1 << id) == 0);
        s.finished |= 1 << id;
        if id == M && s.epoch == PARTITION {
            s.release(1 << P0);
        }
        if id == P1 {
            GATES.fetch_or(P1_FINISHED, Ordering::Release);
        }
        if s.finished == 7 {
            s.finish_epoch(audit);
        }
    });
    dispatch(cpu)
}
