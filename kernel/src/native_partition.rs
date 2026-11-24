//! Static P/Q tables on one physical timer. Only IRQ-masked EL1 mutates State.
use super::{
    ELF, UserFrame, ZERO, arguments, fail, header, init_irq, load_plan, mmu,
    native_vruntime_deadline, native_vruntime_quantum, native_vruntime_start, observe_trap, park,
    result_object, row, ticks, uart_write,
};
use crate::temporal_policy::{CyclicExecutive, WindowDecision};
use core::arch::asm;

const IDLE: usize = 2;
const PROCESSES: usize = mmu::PROCESS_COUNT;
const RESULT_WORDS: usize = 6;

// At most 12 ordinary IRQ-to-window transitions (Q). Each half has
// one writer. Generation one has no predecessor and drains are excluded.
#[cfg(jnox_partition_phase)]
#[unsafe(no_mangle)]
static mut native_partition_irq_phase_v1: [[u64; 17]; 12] = [[0; 17]; 12];

enum Table {
    P(CyclicExecutive<3>),
    Q(CyclicExecutive<4>),
}

impl Table {
    fn new(q: u64, table_q: bool) -> Self {
        if table_q {
            Self::Q(
                CyclicExecutive::new(
                    2,
                    5 * q,
                    [(Some(1), q), (None, q), (Some(0), 2 * q), (Some(1), q)],
                )
                .unwrap_or_else(|| fail(51)),
            )
        } else {
            Self::P(
                CyclicExecutive::new(2, 4 * q, [(Some(0), 2 * q), (Some(1), q), (None, q)])
                    .unwrap_or_else(|| fail(51)),
            )
        }
    }

    fn decision(&self, offset: u64, released: bool) -> WindowDecision {
        let ready = |pid| pid == 0 || released;
        match self {
            Self::P(table) => table.decision(offset, ready),
            Self::Q(table) => table.decision(offset, ready),
        }
        .unwrap_or_else(|| fail(52))
    }
}

struct State {
    table: Table,
    frames: [UserFrame; PROCESSES],
    result: usize,
    q: u64,
    epoch: u64,
    opening: u64,
    closing: u64,
    slots: u64,
    generation: u64,
    released: bool,
    pid: usize,
    drain: bool,
    last_work: [u64; PROCESSES],
    late: u64,
}

static mut STATE: Option<State> = None;

fn sum(a: u64, b: u64) -> u64 {
    a.checked_add(b).unwrap_or_else(|| fail(53))
}

// UserFrame is repr(C), 832 bytes of u64 fields and arrays, with no padding.
// Record the entire incoming/outgoing bank, including SIMD, FP state and TLS.
fn audit_frame(tag: u64, tick: u64, pid: usize, frame: &UserFrame) {
    const _: () = assert!(core::mem::size_of::<UserFrame>() == 104 * 8);
    let words = unsafe { &*(core::ptr::from_ref(frame).cast::<[u64; 104]>()) };
    // Only the IRQ-masked partition writer appends. Publish the cursor after
    // the complete bank so replay sees the same eight adjacent journal rows.
    let journal = unsafe { &mut *(&raw mut super::native_vruntime_journal) };
    let start = journal.header[6] as usize;
    let count = words.len() / 13;
    if start > journal.rows.len() - count {
        header(7, 1);
        fail(1)
    }
    let end = start + count;
    for (record, chunk) in journal.rows[start..end].iter_mut().zip(words.chunks_exact(13)) {
        record[..3].copy_from_slice(&[tag, tick, pid as u64]);
        record[3..].copy_from_slice(chunk);
    }
    header(6, end as u64);
}

impl State {
    fn dispatch(&mut self, opening: u64) -> *const UserFrame {
        #[cfg(jnox_partition_phase)]
        let schedule_before = ticks();
        self.generation += 1;
        self.opening = opening;
        let (owner, pid, closing) = if self.drain {
            // Only A and B normally exit. Idle stays quiescent.
            let pid = (self.generation - self.slots - 1) as usize;
            (pid, pid, sum(ticks(), self.q))
        } else {
            let offset = opening.checked_sub(self.epoch).unwrap_or_else(|| fail(54));
            let decision = self.table.decision(offset, self.released);
            if decision.owner.is_none() {
                self.released = true;
            }
            (
                decision.owner.unwrap_or(IDLE),
                decision.runnable.unwrap_or(IDLE),
                sum(self.epoch, decision.end_tick),
            )
        };
        self.pid = pid;
        self.closing = closing;
        #[cfg(jnox_partition_phase)]
        let mmu_before = ticks();
        unsafe { mmu::switch(pid) }.unwrap_or_else(|_| fail(55));
        let space = mmu::space_id(pid).unwrap_or_else(|| fail(55));
        #[cfg(jnox_partition_phase)]
        let publish_before = ticks();
        mmu::publish_generation(self.generation);
        #[cfg(jnox_partition_phase)]
        let publish_after = ticks();
        unsafe { native_vruntime_deadline = closing };
        let f = &self.frames[pid];
        let selected = ticks();
        #[cfg(jnox_partition_phase)]
        if self.generation >= 2 && !self.drain {
            let index = (self.generation - 2) as usize;
            unsafe {
                (&raw mut native_partition_irq_phase_v1[index][8]).cast::<[u64; 9]>().write_volatile([
                    self.generation, pid as u64, schedule_before, mmu_before,
                    publish_before, publish_after, selected, opening, closing,
                ]);
            }
        }
        if selected >= closing {
            fail(56)
        }
        row(
            21,
            selected,
            pid,
            &[
                self.generation,
                opening,
                closing,
                owner as u64,
                if self.released { 3 } else { 1 },
                self.drain as u64,
                space,
                f.elr,
                f.sp,
                f.spsr,
                f.tls,
                f as *const _ as u64,
            ],
        );
        audit_frame(24, selected, pid, f);
        f
    }

    fn result(&self) -> [u64; RESULT_WORDS] {
        mmu::partition_result(self.pid, self.result).unwrap_or_else(|| fail(57))
    }

    fn trap(&mut self, f: &UserFrame, vector: u64, entry: u64) -> *const UserFrame {
        let p = self.pid;
        #[cfg(jnox_partition_phase)]
        let observe_before = ticks();
        let (start, compare, ctl, iar) =
            observe_trap(p, f, vector, entry, self.generation, &mut self.frames[p]);
        #[cfg(jnox_partition_phase)]
        let result_before = ticks();
        let [role, seen, first, work, checksum, status] = self.result();
        #[cfg(jnox_partition_phase)]
        let audit_before = ticks();
        row(
            22,
            entry,
            p,
            &[
                self.generation,
                start,
                compare,
                ctl,
                iar,
                vector,
                f.elr,
                f.sp,
                f.spsr,
                f.tls,
                f.x[8],
                f.x[0],
            ],
        );
        row(
            23,
            entry,
            p,
            &[self.generation, role, seen, first, work, checksum, status],
        );
        audit_frame(25, entry, p, f);
        if start < self.opening
            || start >= compare
            || compare != self.closing
            || entry < start
            || role != (p + 1) as u64
        {
            fail(58)
        }
        if self.drain {
            if vector != 0x400
                || f.x[8] != 1
                || f.x[0] != 41 + p as u64
                || status != f.x[0]
                || entry >= compare
                || work < self.last_work[p]
                || work.checked_mul(if p == 0 { 3 } else { 5 }) != Some(checksum)
            {
                fail(59)
            }
            if p == 1 {
                header(1, 3);
                uart_write(b"NATIVE PARTITION END\n");
                park()
            }
            return self.dispatch(entry);
        }
        if vector != 0x480
            || seen != self.generation
            || !(start..entry).contains(&first)
            || (p == IDLE && (work != 0 || checksum != 0))
            || (p != IDLE && work <= self.last_work[p])
        {
            fail(60)
        }
        self.last_work[p] = work;
        // The terminal marker reports complete capture. Keep timing failures in RAM.
        if first - self.opening > self.q / 50 {
            self.late += 1;
            header(12, self.late);
        }
        header(11, self.generation);
        if self.generation == self.slots {
            if !mmu::set_end() {
                fail(61)
            }
            self.drain = true;
            header(9, entry);
            return self.dispatch(entry);
        }
        #[cfg(jnox_partition_phase)]
        {
            let audit_after = ticks();
            let index = (self.generation - 1) as usize;
            unsafe {
                (&raw mut native_partition_irq_phase_v1[index][0]).cast::<[u64; 8]>().write_volatile([
                    self.generation, p as u64, entry, observe_before, result_before,
                    audit_before, audit_after, vector,
                ]);
            }
        }
        self.dispatch(self.closing)
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_vruntime_trap(
    frame: *const UserFrame,
    vector: u64,
    entry: u64,
) -> *const UserFrame {
    let state = unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(62)) };
    state.trap(unsafe { &*frame }, vector, entry)
}

pub(super) fn run() -> ! {
    let plan = load_plan();
    let result =
        result_object(ELF, &plan, b"native_partition_result", 48).unwrap_or_else(|| fail(63));
    let table_q = env!("JNOX_WEIGHTED_NATIVE_APP") == "partition-q";
    let names: [&[u8]; PROCESSES] = [b"A\0", b"B\0", b"I\0"];
    let tails = names.map(|name| arguments([b"partition\0", name, b"cycle\0"]));
    init_irq();
    unsafe {
        mmu::install(
            [&plan; PROCESSES],
            tails.each_ref().map(|t| t.as_slice()),
            0,
        )
    }
    .unwrap_or_else(|_| fail(64));
    let frequency: u64;
    unsafe {
        // Permit EL0 counter reads. Timer programming remains privileged.
        asm!("mrs {f},cntfrq_el0", "mrs {k},cntkctl_el1", "orr {k},{k},#1",
             "msr cntkctl_el1,{k}", "isb", f=out(reg) frequency, k=out(reg) _, options(nostack));
    }
    if frequency == 0 || frequency % 1000 != 0 {
        fail(65)
    }
    let q = frequency / 20;
    let table = Table::new(q, table_q);
    let frames = core::array::from_fn(|p| UserFrame {
        tls: (mmu::STACK_TOP - 96 - 16 * p) as u64,
        elr: plan.entry,
        sp: (mmu::STACK_TOP - 80) as u64,
        ..ZERO
    });
    let mut maps = [[0u64; 3]; PROCESSES];
    for p in 0..PROCESSES {
        let space = mmu::space_id(p).unwrap_or_else(|| fail(66));
        let object = mmu::backing_page(p, result).unwrap_or_else(|| fail(66)) as u64;
        let stack = mmu::backing_page(p, mmu::STACK_TOP - 80).unwrap_or_else(|| fail(66)) as u64;
        if maps[..p]
            .iter()
            .any(|old| old[0] == space || old[1] == object || old[2] == stack)
        {
            fail(66)
        }
        maps[p] = [space, object, stack];
        row(
            1,
            ticks(),
            p,
            &[space, object, stack, plan.entry, frames[p].sp],
        );
    }
    let slots = if table_q { 13 } else { 10 };
    header(0, 2);
    header(2, if table_q { 2 } else { 1 });
    header(3, frequency);
    header(4, q);
    header(10, slots);
    header(14, result as u64);
    unsafe {
        // Each scheduled window is at most 2q, so CVAL takes its absolute end.
        native_vruntime_quantum = 3 * q;
        *(&raw mut STATE) = Some(State {
            table,
            frames,
            result,
            q,
            epoch: 0,
            opening: 0,
            closing: 0,
            slots,
            generation: 0,
            released: false,
            pid: 0,
            drain: false,
            last_work: [0; PROCESSES],
            late: 0,
        });
        let state = (*(&raw mut STATE)).as_mut().unwrap();
        let epoch = ticks();
        state.epoch = epoch;
        header(5, epoch);
        header(1, 2);
        native_vruntime_start(state.dispatch(epoch))
    }
}
