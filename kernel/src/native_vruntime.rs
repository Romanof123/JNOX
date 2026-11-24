//! Single CPU, two private EL0 contexts. Only masked EL1 code mutates State.
use crate::{
    native_elf::{self, Permission},
    native_process_mmu as mmu,
    native_v1_note::{native_v1_note, result_object},
    park, uart_write,
    vruntime_policy::{Lease, VruntimeScheduler},
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
};
global_asm!(include_str!("native_vruntime_vectors.S"), options(raw));
unsafe extern "C" {
    fn native_vruntime_start(f: *const UserFrame) -> !;
    static native_vruntime_return_tick: u64;
}
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct UserFrame {
    x: [u64; 31],
    esr: u64,
    spsr: u64,
    elr: u64,
    sp: u64,
    current_el: u64,
    fpcr: u64,
    fpsr: u64,
    q: [[u64; 2]; 32],
    tls: u64,
    tls_ro: u64,
}
const ZERO: UserFrame = UserFrame {
    x: [0; 31],
    esr: 0,
    spsr: 0x340,
    elr: 0,
    sp: 0,
    current_el: 4,
    fpcr: 0,
    fpsr: 0,
    q: [[0; 2]; 32],
    tls: 0,
    tls_ro: 0,
};
const _: () = assert!(size_of::<UserFrame>() == 832 && offset_of!(UserFrame, tls) == 816);
static ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));
#[unsafe(no_mangle)]
static mut native_vruntime_quantum: u64 = 0;
#[unsafe(no_mangle)]
static mut native_vruntime_deadline: u64 = u64::MAX;
#[repr(C)]
struct Journal {
    header: [u64; 16],
    rows: [[u64; 16]; 512],
}
#[unsafe(no_mangle)]
static mut native_vruntime_journal: Journal = Journal {
    header: [0; 16],
    rows: [[0; 16]; 512],
};
fn header(i: usize, v: u64) {
    unsafe {
        ptr::write_volatile(
            (&raw mut native_vruntime_journal.header)
                .cast::<u64>()
                .add(i),
            v,
        )
    }
}
fn row(tag: u64, t: u64, p: usize, fields: &[u64]) {
    unsafe {
        let j = &mut *(&raw mut native_vruntime_journal);
        let i = j.header[6] as usize;
        if i >= j.rows.len() || fields.len() > 13 {
            header(7, 1);
            fail(1)
        }
        let r = &mut j.rows[i];
        r[..3].copy_from_slice(&[tag, t, p as u64]);
        r[3..3 + fields.len()].copy_from_slice(fields);
        header(6, (i + 1) as u64);
    }
}
fn ticks() -> u64 {
    let t;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)t,options(nostack)) };
    t
}
fn fail(code: u64) -> ! {
    header(8, code);
    header(1, 4);
    uart_write(b"NATIVE VRUNTIME FAIL\n");
    park()
}
#[unsafe(no_mangle)]
extern "C" fn native_vruntime_unexpected() -> ! {
    fail(2)
}
const DIST: usize = 0x0800_0000;
const CPU: usize = 0x0801_0000;
fn read(a: usize) -> u32 {
    unsafe { ptr::read_volatile(a as *const u32) }
}
fn write(a: usize, v: u32) {
    unsafe { ptr::write_volatile(a as *mut u32, v) }
}
fn init_irq() {
    unsafe {
        asm!(
            "msr daifset,#15",
            "msr cntp_ctl_el0,xzr",
            "msr cntv_ctl_el0,xzr",
            "isb",
            options(nostack)
        )
    }
    write(DIST, 0);
    write(CPU, 0);
    write(DIST + 0x80, read(DIST + 0x80) & !(1 << 30));
    unsafe { ptr::write_volatile((DIST + 0x400 + 30) as *mut u8, 0x80) };
    write(DIST + 0x280, 1 << 30);
    write(DIST + 0x100, 1 << 30);
    write(CPU + 4, 0xff);
    write(DIST, 1);
    write(CPU, 1);
    unsafe { asm!("dsb sy", "isb", options(nostack)) }
}
fn tail(p: usize, wake: bool) -> [u8; 80] {
    let mut bytes = [0; 80];
    let sp = mmu::STACK_TOP - bytes.len();
    let args: [&[u8]; 3] = [
        b"weighted\0",
        if p == 0 { b"A\0" } else { b"B\0" },
        if wake { b"wake\0" } else { b"share\0" },
    ];
    bytes[..8].copy_from_slice(&3u64.to_le_bytes());
    let mut offset = 40;
    for (i, a) in args.iter().enumerate() {
        bytes[8 + i * 8..16 + i * 8].copy_from_slice(&((sp + offset) as u64).to_le_bytes());
        bytes[offset..offset + a.len()].copy_from_slice(a);
        offset += a.len()
    }
    bytes
}
struct State {
    policy: VruntimeScheduler<2>,
    frames: [UserFrame; 2],
    live: Option<Lease>,
    frequency: u64,
    epoch: u64,
    q: u64,
    wake: bool,
    due: Option<u64>,
    wakes: u64,
    closures: u64,
    ended: bool,
    exited: [bool; 2],
    result: usize,
}
static mut STATE: Option<State> = None;
impl State {
    fn service(&self, p: usize) -> u64 {
        self.policy.service(p).unwrap_or_else(|| fail(3))
    }
    fn log_frame(&self, p: usize, seq: u64, sel: u64, t: u64) {
        let f = &self.frames[p];
        row(
            11,
            t,
            p,
            &[
                seq,
                sel,
                f as *const _ as u64,
                f.x[1],
                f.x[2],
                f.x[3],
                f.x[20],
                f.x[21],
                f.x[22],
                f.x[29],
                f.x[30],
                f.tls_ro,
                f.esr,
            ],
        );
        row(12, t, p, &[seq, sel, f.x[0], f.x[19], f.tls]);
    }
    fn select(&mut self) -> *const UserFrame {
        let l = self.policy.dispatch().unwrap_or_else(|| fail(4));
        let p = l.task;
        unsafe { mmu::switch(p) }.unwrap_or_else(|_| fail(5));
        if mmu::active() != Some(p) {
            fail(5)
        }
        let f = &self.frames[p];
        let (whole, rem, _) = self.policy.runtime(p).unwrap().parts();
        let due = self.due.unwrap_or(u64::MAX);
        unsafe { native_vruntime_deadline = due }
        let t = ticks();
        row(
            2,
            t,
            p,
            &[
                l.sequence,
                mmu::space_id(p).unwrap_or_else(|| fail(6)),
                f.elr,
                f.sp,
                f.spsr,
                f.x[0],
                f.x[19],
                f.tls,
                due,
                self.service(0),
                self.service(1),
                whole,
                rem,
            ],
        );
        self.log_frame(p, l.sequence, 1, t);
        self.live = Some(l);
        f
    }
    fn end(&mut self, p: usize, t: u64) {
        if self.ended {
            return;
        }
        if !mmu::set_end() {
            fail(28)
        }
        self.ended = true;
        header(9, t);
        row(7, t, p, &[self.closures, self.wakes]);
    }
    fn result_words(&self, p: usize) -> [u64; 28] {
        let mut bytes = [0u8; 224];
        if !mmu::copy_from_user(p, self.result, &mut bytes) {
            fail(7)
        }
        core::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
    }
    fn clock(&self, p: usize, out: usize) -> i64 {
        let ns = u128::from(ticks() - self.epoch) * 1_000_000_000 / u128::from(self.frequency);
        let Ok(sec) = i64::try_from(ns / 1_000_000_000) else {
            return -75;
        };
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&sec.to_le_bytes());
        bytes[8..].copy_from_slice(&((ns % 1_000_000_000) as i64).to_le_bytes());
        if mmu::copy_to_user(p, out, &bytes) {
            0
        } else {
            -14
        }
    }
    fn sleep(&mut self, p: usize, addr: usize) -> i64 {
        let mut bytes = [0; 16];
        if !mmu::copy_from_user(p, addr, &mut bytes) {
            return -14;
        }
        let sec = i64::from_le_bytes(bytes[..8].try_into().unwrap());
        let ns = i64::from_le_bytes(bytes[8..].try_into().unwrap());
        if sec < 0 || !(0..1_000_000_000).contains(&ns) {
            return -22;
        }
        let input = sec as u128 * 1_000_000_000 + ns as u128;
        let Some(target) = input
            .checked_mul(self.frequency as u128)
            .and_then(|n| u64::try_from(n.div_ceil(1_000_000_000)).ok())
            .and_then(|n| self.epoch.checked_add(n))
        else {
            return -75;
        };
        let start = ticks();
        if target <= start {
            return 0;
        }
        if !self.wake || p != 1 || self.due.is_some() || self.wakes >= 3 || !self.policy.sleep(p) {
            return -22;
        }
        self.due = Some(target);
        let (a, b, c) = self.policy.floor().parts();
        let (d, e, f) = self.policy.runtime(p).unwrap().parts();
        row(
            5,
            start,
            p,
            &[target, start, self.service(p), a, b, c, d, e, f],
        );
        0
    }
    fn syscall(&mut self, p: usize, entry: u64, w: &[u64; 28]) {
        let id = self.frames[p].x[8];
        let args: [u64; 4] = self.frames[p].x[..4].try_into().unwrap();
        let ret = match id {
            1 => {
                let status = args[0];
                row(
                    8,
                    entry,
                    p,
                    &[status, w[3], w[4], w[5], w[6], w[7], w[8], w[27]],
                );
                if p == 1 && self.wake {
                    for r in 0..3 {
                        let mut a = [0; 7];
                        a[0] = r as u64;
                        a[1..].copy_from_slice(&w[9 + r * 6..15 + r * 6]);
                        row(10, entry, p, &a)
                    }
                }
                if !self.ended
                    || status != 41 + p as u64
                    || w[27] != status
                    || !self.policy.sleep(p)
                {
                    fail(8)
                }
                self.exited[p] = true;
                0
            }
            6 if args[0] == 1 => self.clock(p, args[1] as usize),
            7 if args[0] == 1 && args[1] == 1 && args[3] == 0 => self.sleep(p, args[2] as usize),
            6 | 7 => -22,
            _ => -38,
        };
        self.frames[p].x[0] = ret as u64;
        row(
            4,
            entry,
            p,
            &[id, args[0], args[1], args[2], args[3], ret as u64],
        );
    }
    fn trap(&mut self, f: &UserFrame, vector: u64, entry: u64) -> *const UserFrame {
        let l = self.live.take().unwrap_or_else(|| fail(9));
        let p = l.task;
        if mmu::active() != Some(p)
            || f.current_el != 4
            || f.spsr & 0x3ff != 0x340
            || !(mmu::STACK_BOTTOM as u64..=mmu::STACK_TOP as u64).contains(&f.sp)
            || f.sp % 16 != 0
        {
            fail(10)
        }
        self.frames[p] = *f;
        let start = unsafe { ptr::read_volatile(&raw const native_vruntime_return_tick) };
        // Reading GICC_IAR acknowledges the level-sensitive PPI. Keep its
        // source asserted through acknowledgement. SVC never reads the IAR.
        let iar = if vector == 0x480 {
            u64::from(read(CPU + 12))
        } else {
            u64::MAX
        };
        let compare: u64;
        let ctl: u64;
        unsafe {
            asm!("mrs {},cntp_cval_el0","mrs {},cntp_ctl_el0","msr cntp_ctl_el0,xzr","isb",out(reg)compare,out(reg)ctl,options(nostack))
        }
        if vector == 0x480 {
            if iar & 1023 != 30 || ctl & 7 != 5 || entry < compare {
                // Failure-only row: no interval, service or return is credited.
                row(13, entry, p, &[l.sequence, vector, iar, ctl, compare, start, f.elr]);
                fail(11)
            }
            write(CPU + 16, iar as u32);
            unsafe { asm!("dsb sy", options(nostack)) };
        } else if vector != 0x400 || f.esr >> 26 != 0x15 || f.esr & 0xffff != 0x4a01 {
            fail(12)
        }
        if start == 0
            || compare
                != start
                    .checked_add(self.q)
                    .unwrap_or_else(|| fail(13))
                    .min(self.due.unwrap_or(u64::MAX))
        {
            fail(14)
        }
        let elapsed = entry.checked_sub(start).unwrap_or_else(|| fail(15));
        if !self.policy.charge(l, elapsed) {
            fail(16)
        }
        row(
            3,
            entry,
            p,
            &[
                l.sequence,
                vector,
                start,
                compare,
                ctl,
                iar,
                f.elr,
                f.sp,
                f.spsr,
                f.x[8],
                elapsed,
                self.service(0),
                self.service(1),
            ],
        );
        self.log_frame(p, l.sequence, 0, entry);
        let w = self.result_words(p);
        row(
            9,
            entry,
            p,
            &[l.sequence, w[3], w[4], w[5], w[6], w[7], w[8]],
        );
        if vector == 0x400 {
            self.syscall(p, entry, &w)
        } else if self.wake {
            if let Some(target) = self.due.filter(|d| *d <= entry) {
                let (a, b, c) = self.policy.floor().parts();
                let (d, e, _) = self.policy.runtime(1).unwrap().parts();
                if !self.policy.wake(1) {
                    fail(17)
                }
                self.due = None;
                self.wakes += 1;
                let t = ticks();
                let (g, h, _) = self.policy.runtime(1).unwrap().parts();
                row(
                    6,
                    t,
                    1,
                    &[target, entry, self.service(1), a, b, c, d, e, g, h],
                );
                header(10, self.wakes);
                if self.wakes == 3 {
                    self.end(p, t)
                }
            }
        } else if !self.ended {
            self.closures += 1;
            header(11, self.closures);
            if self.closures == 36 {
                self.end(p, ticks())
            }
        }
        if self.exited == [true, true] {
            header(12, self.service(0));
            header(13, self.service(1));
            header(1, 3);
            uart_write(b"NATIVE VRUNTIME END\n");
            park()
        }
        self.select()
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_vruntime_trap(
    frame: *const UserFrame,
    vector: u64,
    entry: u64,
) -> *const UserFrame {
    let s = unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(18)) };
    s.trap(unsafe { &*frame }, vector, entry)
}
pub(super) fn run() -> ! {
    let plan = native_elf::plan(ELF, 0x400000..0x800000).unwrap_or_else(|_| fail(19));
    if plan.entry != 0x400000
        || !plan.segments().iter().map(|s| s.permission).eq([
            Permission::ReadExecute,
            Permission::Read,
            Permission::ReadWrite,
        ])
        || native_v1_note(ELF, &plan).is_none()
    {
        fail(20)
    }
    let result =
        result_object(ELF, &plan, b"native_weighted_result", 224).unwrap_or_else(|| fail(21));
    let wake = env!("JNOX_WEIGHTED_NATIVE_APP") == "weighted-wake";
    let tails = [tail(0, wake), tail(1, wake)];
    init_irq();
    unsafe { mmu::install([&plan, &plan], [&tails[0], &tails[1]], 0).unwrap_or_else(|_| fail(22)) }
    let frequency: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)frequency,options(nostack)) };
    if frequency == 0 || frequency % 400 != 0 {
        fail(23)
    }
    let q = frequency / 100;
    let epoch = ticks();
    let frames = core::array::from_fn(|p| UserFrame {
        tls: (mmu::STACK_TOP - 96 - 16 * p) as u64,
        elr: plan.entry,
        sp: (mmu::STACK_TOP - 80) as u64,
        ..ZERO
    });
    let state = State {
        policy: VruntimeScheduler::new([1, 2]).unwrap(),
        frames,
        live: None,
        frequency,
        epoch,
        q,
        wake,
        due: None,
        wakes: 0,
        closures: 0,
        ended: false,
        exited: [false; 2],
        result,
    };
    header(0, 1);
    header(1, 1);
    header(2, if wake { 2 } else { 1 });
    header(3, frequency);
    header(4, q);
    header(5, epoch);
    header(14, result as u64);
    let maps = [0, 1].map(|p| {
        let space = mmu::space_id(p).unwrap_or_else(|| fail(24));
        let object = mmu::backing_page(p, result).unwrap_or_else(|| fail(25));
        let stack = mmu::backing_page(p, mmu::STACK_TOP - 80).unwrap_or_else(|| fail(26));
        row(
            1,
            epoch,
            p,
            &[
                space,
                object as u64,
                stack as u64,
                plan.entry,
                (mmu::STACK_TOP - 80) as u64,
            ],
        );
        (space, object, stack)
    });
    if maps[0].0 == maps[1].0 || maps[0].1 == maps[1].1 || maps[0].2 == maps[1].2 {
        fail(27)
    }
    unsafe {
        native_vruntime_quantum = q;
        *(&raw mut STATE) = Some(state);
        let f = (*(&raw mut STATE)).as_mut().unwrap().select();
        header(1, 2);
        native_vruntime_start(f)
    }
}
