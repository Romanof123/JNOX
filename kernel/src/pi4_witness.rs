//! One CPU. Each dispatch owns a unique pair of worker observation cells.
//! Selection writers run masked; the exporter runs only on the terminal idle stack.
use super::*;
use core::fmt::{self, Write};
use core::sync::atomic::{
    AtomicU64,
    Ordering::{Acquire, Relaxed, Release},
};

const CAP: usize = 64;
#[cfg_attr(jnox_rr_inner, unsafe(export_name = "pi4_rr_dispatch_generation"))]
static GEN: AtomicU64 = AtomicU64::new(0);
static mut BOOT: u64 = 0;
static mut ENTRIES: [u64; 3] = [0; 3];
static mut EDGES: [[u64; 5]; CAP] = [[0; 5]; CAP];
static mut SAMPLES: [[[u64; 7]; 2]; CAP] = [[[0; 7]; 2]; CAP];
static ACTIVATIONS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static LOST: AtomicU64 = AtomicU64::new(0);

fn affinity() -> u64 {
    let cpu;
    unsafe { asm!("mrs {},mpidr_el1",out(reg)cpu,options(nostack,preserves_flags)) };
    cpu
}
fn level() -> u64 {
    let el: u64;
    unsafe { asm!("mrs {},CurrentEL",out(reg)el,options(nostack,preserves_flags)) };
    el >> 2
}
pub(super) fn compare() -> u64 {
    let c;
    unsafe { asm!("mrs {},cntp_cval_el0",out(reg)c,options(nostack,preserves_flags)) };
    c
}
pub(super) fn init(tasks: &[Task]) {
    if tasks.len() != 3 {
        fail()
    }
    unsafe {
        BOOT = counter();
        ENTRIES = core::array::from_fn(|i| tasks[i].entry as usize as u64);
    }
}
pub(super) fn ambiguous() {
    LOST.fetch_add(1, Relaxed);
}
pub(super) fn invalidate() {
    GEN.store(0, Release)
}
pub(super) fn selected(i: usize, pre_cval: u64) {
    let [a, b] = crate::fixed_apps::witness_progress();
    let core = affinity();
    unsafe { EDGES[i] = [pre_cval, core, a, b, counter()] };
    GEN.store(i as u64 + 1, Release);
}
#[inline(always)]
pub(crate) fn generation() -> u64 {
    GEN.load(Acquire)
}

#[inline(never)]
#[unsafe(no_mangle)]
fn pi4_witness_site() -> (u64, u64, u64) {
    let (pc, sp, time);
    unsafe {
        asm!("2:","adr {pc},2b","mov {sp},sp","isb","mrs {time},cntpct_el0",
        pc=out(reg)pc,sp=out(reg)sp,time=out(reg)time,options(nostack,preserves_flags))
    };
    (pc, sp, time)
}

pub(crate) fn activation(role: usize) {
    let value = ACTIVATIONS[role].fetch_add(1, Relaxed) + 1;
    // An IRQ can preempt the entry probe. One bounded retry samples the
    // resumed generation; repeated ambiguity is explicit capture loss.
    for _ in 0..2 {
        if probe(generation(), role, 0, value) {
            return;
        }
    }
    LOST.fetch_add(1, Relaxed);
}
#[inline(always)]
pub(crate) fn probe(sequence: u64, role: usize, kind: usize, progress: u64) -> bool {
    let (pc, sp, t) = pi4_witness_site();
    let core = affinity();
    if sequence == 0 || sequence > CAP as u64 || generation() != sequence {
        return false;
    }
    // The sample interval ends before the generation recheck. A subsequent
    // IRQ writes distinct cells. This task alone completes the old cell.
    let dst = unsafe { &raw mut SAMPLES[sequence as usize - 1][kind] };
    unsafe {
        if ptr::read_volatile(dst)[0] != 0 {
            LOST.fetch_add(1, Relaxed);
            return true;
        }
        ptr::write_volatile(dst, [sequence, role as u64, core, pc, sp, t, progress]);
    }
    true
}

struct Serial;
impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::uart_write(s.as_bytes());
        Ok(())
    }
}
struct Export {
    hash: u64,
    rows: u64,
}
impl Export {
    fn word(&mut self, v: u64) {
        for byte in v.to_le_bytes() {
            self.hash = (self.hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        }
    }
    fn row(&mut self, tag: u64, fields: &[u64]) {
        let _ = write!(Serial, "PW {tag}");
        self.word(tag);
        for &v in fields {
            self.word(v);
            let _ = write!(Serial, " {v}");
        }
        crate::uart_write(b"\n");
        self.rows += 1;
    }
}
fn slot_role(slot: usize) -> u64 {
    if slot == IDLE {
        return 3;
    }
    if cfg!(jnox_app = "fixed-rr") {
        [0, 2, 1][slot]
    } else {
        slot as u64
    }
}
#[unsafe(no_mangle)]
pub(super) extern "C" fn pi4_witness_terminal() -> ! {
    unsafe { asm!("msr daifset,#0xf", "isb", options(nostack, preserves_flags)) };
    let (pc, sp, t) = pi4_witness_site();
    let sequence = generation();
    let (n, lost) = trace::status();
    let mut out = Export {
        hash: 0xcbf29ce484222325,
        rows: 0,
    };
    let id = env!("PI4_WITNESS_ID")
        .parse::<u64>()
        .unwrap_or_else(|_| fail());
    out.row(
        0,
        &[
            if cfg!(jnox_rr_inner) { 3 } else if cfg!(jnox_rr_handoff) { 2 } else { 1 },
            id,
            unsafe { BOOT },
            u64::from(cfg!(jnox_app = "fixed-rr")),
            affinity(),
            level(),
            frequency(),
            CAP as u64,
            (2 * CAP) as u64,
            size_of::<Frame>() as u64,
            n,
            lost,
            LOST.load(Acquire),
        ],
    );
    for slot in [0, 1, 2, IDLE] {
        out.row(
            1,
            &[
                slot as u64,
                slot_role(slot),
                stack_base(slot).addr() as u64,
                stack_base(slot).addr() as u64 + STACK_BYTES as u64,
                if slot == IDLE {
                    fixed_runtime_idle as *const () as usize as u64
                } else {
                    unsafe { ENTRIES[slot] }
                },
            ],
        );
    }
    for i in 0..n as usize {
        let mut fields = [0; 25];
        fields[0] = i as u64 + 1;
        fields[1..20].copy_from_slice(&trace::row(i));
        fields[20..].copy_from_slice(&unsafe { EDGES[i] });
        out.row(2, &fields);
        for kind in 0..2 {
            let r = unsafe { SAMPLES[i][kind] };
            if r[0] != 0 {
                let [seq, role, cpu, probe, stack, time, value] = r;
                out.row(3, &[seq, kind as u64, role, cpu, probe, stack, time, value]);
            }
        }
    }
    #[cfg(jnox_rr_handoff)]
    handoff::export(&mut out);
    out.row(
        4,
        &[
            sequence,
            affinity(),
            pc,
            sp,
            t,
            pi4_witness_terminal as *const () as usize as u64,
        ],
    );
    let _ = writeln!(Serial, "PW END {} {}", out.rows, out.hash);
    crate::park()
}
pub(super) use pi4_witness_terminal as terminal;

// A single writer owns each role. Publication is last, after the generation
// recheck. An IRQ after validation cannot change this immutable sample.
#[cfg(jnox_rr_handoff)]
pub(crate) mod handoff {
    use super::*;
    static STATE: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
    static mut DATA: [[u64; 10]; 2] = [[0; 10]; 2];
    #[cfg(jnox_rr_inner)]
    static mut INNER: [[u64; 2]; 2] = [[0; 2]; 2];

    #[inline(always)]
    fn ticks() -> u64 {
        let t;
        unsafe { asm!("isb", "mrs {},cntpct_el0", "isb", out(reg)t, options(nostack,preserves_flags)) };
        t
    }

    #[inline(always)]
    fn finish(role: usize, begin: u64, lo: u64, mailbox: &AtomicU64, value: u64, pc: u64, sp: u64, #[cfg(jnox_rr_inner)] inner: [u64; 2]) {
        let cpu = affinity();
        let hi = ticks();
        let end = generation();
        if STATE[role].load(Relaxed) != 0 {
            STATE[role].store(3, Release);
            return;
        }
        #[cfg(jnox_rr_inner)]
        unsafe { INNER[role] = inner };
        unsafe { DATA[role] = [begin,end,lo,hi,mailbox.as_ptr() as u64,value,pc,sp,cpu,role as u64] };
        STATE[role].store(if begin != 0 && begin == end { 1 } else { 2 }, Release);
    }

    #[inline(never)]
    #[unsafe(no_mangle)]
    pub(crate) fn pi4_rr_publish(mailbox: &AtomicU64, value: u64) {
        let begin = generation();
        let lo = ticks();
        let (pc, sp): (u64, u64);
        #[cfg(not(jnox_rr_inner))]
        unsafe { asm!("adr {pc},2f","mov {sp},sp","2:","stlr {v},[{m}]",
            pc=out(reg)pc,sp=out(reg)sp,v=in(reg)value,m=in(reg)mailbox.as_ptr(),
            options(nostack,preserves_flags)) };
        #[cfg(jnox_rr_inner)]
        let (before, after): (u64, u64);
        #[cfg(jnox_rr_inner)]
        unsafe {
            asm!("adrp {generation},{gen_word}", "add {generation},{generation},:lo12:{gen_word}",
                 "adr {pc},2f", "mov {sp},sp",
                 "ldar {before},[{generation}]", "2:", "stlr {v},[{mail}]", "ldar {after},[{generation}]",
                 pc=out(reg)pc, sp=out(reg)sp, before=out(reg)before, after=out(reg)after,
                 generation=out(reg)_, gen_word=sym GEN, mail=in(reg)mailbox.as_ptr(), v=in(reg)value,
                 options(nostack,preserves_flags));
        }
        finish(0,begin,lo,mailbox,value,pc,sp, #[cfg(jnox_rr_inner)] [before,after]);
    }

    #[inline(never)]
    #[unsafe(no_mangle)]
    pub(crate) fn pi4_rr_consume(mailbox: &AtomicU64) -> u64 {
        let begin = generation();
        let lo = ticks();
        let (value, pc, sp): (u64, u64, u64);
        #[cfg(not(jnox_rr_inner))]
        unsafe { asm!("adr {pc},2f","mov {sp},sp","2:","ldar {v},[{m}]",
            pc=out(reg)pc,sp=out(reg)sp,v=out(reg)value,m=in(reg)mailbox.as_ptr(),
            options(nostack,preserves_flags)) };
        #[cfg(jnox_rr_inner)]
        let (before, after): (u64, u64);
        #[cfg(jnox_rr_inner)]
        unsafe {
            asm!("adrp {generation},{gen_word}", "add {generation},{generation},:lo12:{gen_word}",
                 "adr {pc},2f", "mov {sp},sp",
                 "ldar {before},[{generation}]", "2:", "ldar {v},[{mail}]", "ldar {after},[{generation}]",
                 pc=out(reg)pc, sp=out(reg)sp, before=out(reg)before, after=out(reg)after,
                 generation=out(reg)_, gen_word=sym GEN, mail=in(reg)mailbox.as_ptr(), v=out(reg)value,
                 options(nostack,preserves_flags));
        }
        if value != 0 { finish(1,begin,lo,mailbox,value,pc,sp, #[cfg(jnox_rr_inner)] [before,after]); }
        value
    }

    pub(crate) use pi4_rr_consume as consume;
    pub(crate) use pi4_rr_publish as publish;

    pub(super) fn export(out: &mut Export) {
        for (role, state) in STATE.iter().enumerate() {
            let status = state.load(Acquire);
            let mut fields = [0; 12];
            fields[0] = role as u64;
            fields[1] = status;
            fields[2..].copy_from_slice(&unsafe { DATA[role] });
            out.row(5, &fields);
        }
        #[cfg(jnox_rr_inner)]
        for (role, state) in STATE.iter().enumerate() {
            let outer_status = state.load(Acquire);
            let [before, after] = unsafe { INNER[role] };
            let status = match outer_status {
                0 | 3 => outer_status,
                _ if before != 0 && before == after => 1,
                _ => 2,
            };
            let operation_pc = unsafe { DATA[role][6] };
            out.row(6, &[role as u64, status, before, after, operation_pc]);
        }
    }
}
