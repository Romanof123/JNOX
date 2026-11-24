//! Aligned three-level reservations on one physical-timer-driven EL1 CPU.
use super::context::*;
use super::hierarchy_policy::Hierarchy;
use super::{park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

const IDLE: usize = 4;
const SLOTS: usize = 12;
const PERIOD: u64 = 6;

unsafe extern "C" {
    fn h_leaf0();
    fn h_leaf1();
    fn h_leaf2();
    fn h_leaf3();
    fn h_idle();
    static h_leaf0_wake: u8;
    static h_leaf1_wake: u8;
    static h_leaf2_wake: u8;
    static h_leaf3_wake: u8;
    static h_leaf0_work: u8;
    static h_leaf1_work: u8;
    static h_leaf2_work: u8;
    static h_leaf3_work: u8;
    static h_idle_work: u8;
    static h_leaf0_end: u8;
    static h_leaf1_end: u8;
    static h_leaf2_end: u8;
    static h_leaf3_end: u8;
    static h_idle_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("hierarchy.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym irq, svc = sym wait, fail = sym fail,
);

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Slot {
    slot: u64,
    parent: u64,
    child: u64,
    owner: u64,
    ready_mask: u64,
    grant_start: u64,
    grant_length: u64,
    parent_left: u64,
    child_left: u64,
    leaf_left: u64,
    selected_pc: u64,
    saved_pc: u64,
    saved_frame: u64,
    start_counter: u64,
    stop_counter: u64,
    work: u64,
}

#[repr(C)]
struct Witness {
    count: u64,
    tick_counts: u64,
    epoch_counter: u64,
    wait_mask: u64,
    irq_count: u64,
    pass: u64,
    slots: [Slot; SLOTS],
}

#[unsafe(no_mangle)]
static mut hierarchy_witness: Witness = unsafe { core::mem::zeroed() };
static mut STACKS: [Stack; 5] = [const { Stack([0; STACK_BYTES]) }; 5];
static mut BENCH: Option<Bench> = None;

struct Bench {
    tree: Hierarchy<2, 4, 4>,
    frames: [*mut Frame; 5],
    progress: [u64; 5],
    current: usize,
    ready: [bool; 4],
    slot: usize,
    compare: u64,
    tick: u64,
    grant: Slot,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"HIERARCHY FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

fn arm(compare: u64) {
    unsafe {
        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
             in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
    };
}

fn top(id: usize) -> *mut u8 {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// Access is confined to the masked boot CPU and its exception handlers.
fn state<R>(f: impl FnOnce(&mut Bench, &mut Witness) -> R) -> R {
    unsafe {
        f(
            (*(&raw mut BENCH)).as_mut().unwrap_or_else(|| fail()),
            &mut *(&raw mut hierarchy_witness),
        )
    }
}

impl Bench {
    fn select(&mut self, id: usize) -> *mut Frame {
        self.current = id;
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        frame
    }

    fn save(&mut self, frame: *mut Frame) -> &Frame {
        let id = self.current;
        require(
            self.frames[id].is_null() && frame.addr() == top(id).addr() - 16 - size_of::<Frame>(),
        );
        self.frames[id] = frame;
        let saved = unsafe { &*frame };
        let cookie = 0x610 + id as u64;
        require(saved.gpr[19] == cookie && saved.tpidr_el1 == cookie);
        require(unsafe { ptr::read_volatile(top(id).sub(16).cast::<u64>()) } == cookie);
        saved
    }

    fn save_irq(&mut self, frame: *mut Frame) -> (u64, u64) {
        let id = self.current;
        let begin = [
            &raw const h_leaf0_work,
            &raw const h_leaf1_work,
            &raw const h_leaf2_work,
            &raw const h_leaf3_work,
            &raw const h_idle_work,
        ][id]
            .addr();
        let end = [
            &raw const h_leaf0_end,
            &raw const h_leaf1_end,
            &raw const h_leaf2_end,
            &raw const h_leaf3_end,
            &raw const h_idle_end,
        ][id]
            .addr();
        let saved = self.save(frame);
        require(saved.spsr & 0x8f == 5 && (begin..end).contains(&(saved.elr as usize)));
        let (pc, work) = (saved.elr, saved.gpr[20]);
        require(work > self.progress[id]);
        self.progress[id] = work;
        (pc, work)
    }

    fn dispatch_slot(&mut self, w: &mut Witness) -> *mut Frame {
        let slot = self.slot as u64;
        if slot == self.grant.grant_start + self.grant.grant_length {
            let (parent, child, owner, length) = self
                .tree
                .grant_at(slot, PERIOD, &self.ready)
                .unwrap_or_else(|| fail());
            let (parent_left, child_left, leaf_left) = self
                .tree
                .remaining_at(slot, parent, child, owner)
                .unwrap_or_else(|| fail());
            self.grant = Slot {
                parent: parent as u64,
                child: child as u64,
                owner: owner as u64,
                grant_start: slot,
                grant_length: length,
                parent_left,
                child_left,
                leaf_left,
                ..Slot::default()
            };
        }
        let expected = match slot % PERIOD {
            0 => (0, 1, 1, 1, 2, 0, 5),
            1 | 2 => (0, 2, 2, 2, 0, 2, 4),
            _ => (1, 3, 3, 3, 3, 3, 3),
        };
        let g = &self.grant;
        require(
            (
                g.parent,
                g.child,
                g.owner,
                g.grant_length,
                g.parent_left,
                g.child_left,
                g.leaf_left,
            ) == expected,
        );
        require(self.ready == [false, true, true, true]);
        if slot == 3 {
            uart_write(b"HIERARCHY THROTTLE\n");
        }
        if slot == 6 {
            uart_write(b"HIERARCHY REFILL\n");
        }
        let selected = self.select(self.grant.owner as usize);
        let ready_mask = self
            .ready
            .iter()
            .enumerate()
            .fold(0, |mask, (id, &ready)| mask | (u64::from(ready) << id));
        let start_counter = counter();
        require(start_counter < self.compare);
        w.slots[self.slot] = Slot {
            slot,
            ready_mask,
            selected_pc: unsafe { (*selected).elr },
            start_counter,
            ..self.grant
        };
        arm(self.compare);
        selected
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nostack, preserves_flags)) };
    let tick = freq / 20;
    require(tick > 0);
    let entries: [unsafe extern "C" fn(); 5] = [h_leaf0, h_leaf1, h_leaf2, h_leaf3, h_idle];
    let frames = core::array::from_fn(|id| unsafe {
        let f = top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*f).elr = entries[id] as usize as u64;
        (*f).spsr = 0x3c5;
        f
    });
    let tree = Hierarchy::new(
        0,
        PERIOD,
        [3, 6],
        [(0, 6), (0, 1), (0, 4), (1, 6)],
        [(0, 6), (1, 6), (2, 6), (3, 6)],
    )
    .unwrap_or_else(|| fail());
    unsafe {
        BENCH = Some(Bench {
            tree,
            frames,
            progress: [0; 5],
            current: IDLE,
            ready: [false; 4],
            slot: 0,
            compare: 0,
            tick,
            grant: Slot::default(),
        });
        sched_start_frame(state(|b, w| {
            w.tick_counts = tick;
            b.select(0)
        }))
    }
}

extern "C" fn wait(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && esr & 0xffff == 2);
    state(|b, w| {
        let id = b.current;
        require(id < IDLE && w.wait_mask == (1 << id) - 1);
        let wake = [
            &raw const h_leaf0_wake,
            &raw const h_leaf1_wake,
            &raw const h_leaf2_wake,
            &raw const h_leaf3_wake,
        ][id];
        require(b.save(frame).elr == wake.addr() as u64);
        w.wait_mask |= 1 << id;
        if id + 1 == IDLE {
            uart_write(b"HIERARCHY WAIT\n");
            b.compare = counter().checked_add(b.tick).unwrap_or_else(|| fail());
            w.epoch_counter = b.compare;
            arm(b.compare);
        }
        b.select(id + 1)
    })
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
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
    let stop = counter();
    state(|b, w| {
        let next = b.compare.checked_add(b.tick).unwrap_or_else(|| fail());
        // Fail closed on a whole missed slot. No synthetic catch-up billing.
        require((b.compare..next).contains(&stop));
        let (pc, work) = b.save_irq(frame);
        w.irq_count += 1;
        if b.current == IDLE {
            require(w.wait_mask == 15 && w.irq_count == 1);
            b.ready = [false, true, true, true];
        } else {
            let row = &mut w.slots[b.slot];
            require(row.owner == b.current as u64);
            row.saved_pc = pc;
            row.saved_frame = frame.addr() as u64;
            row.stop_counter = stop;
            row.work = work;
            b.slot += 1;
            w.count = b.slot as u64;
        }
        if b.slot == SLOTS {
            require(w.irq_count == 13);
            w.pass = 1;
            uart_write(b"HIERARCHY PASS\n");
            park()
        }
        b.compare = next;

        b.dispatch_slot(w)
    })
}
