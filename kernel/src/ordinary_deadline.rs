//! Single-CPU ordinary deadline dispatch. Data is printed after stopping the clock.
use super::{
    context::*,
    deadline_jobs::{self as jobs, HORIZON, Job, N},
    deadline_policy::{DeadlinePolicy, DeadlineScheduler},
    park, uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
    sync::atomic::Ordering,
};

const IDLE: usize = N;
const CAP: usize = 24;
static mut STACKS: [Stack; N + 1] = [const { Stack([0; STACK_BYTES]) }; N + 1];
#[unsafe(no_mangle)]
static mut ordinary_deadline_rows: [Row; CAP] = [EMPTY; CAP];
#[unsafe(no_mangle)]
static mut ordinary_deadline_row_count: u64 = 0;
static mut STATE: Option<Runtime> = None;
#[unsafe(no_mangle)]
static mut deadline_restore_count: u64 = 0;
unsafe extern "C" {
    fn ordinary_deadline_return();
    static ordinary_deadline_after_svc: u8;
}
global_asm!(".set JNOX_ORDINARY_DEADLINE, 1",include_str!("context.S"),
    ".global ordinary_deadline_return","ordinary_deadline_return:","svc #1",
    ".global ordinary_deadline_after_svc","ordinary_deadline_after_svc:","b {fail}",
    frame_size=const size_of::<Frame>(),elr=const offset_of!(Frame,elr),fpcr=const offset_of!(Frame,fpcr),
    tpidr_el1=const offset_of!(Frame,tpidr_el1),tpidr_el0=const offset_of!(Frame,tpidr_el0),
    simd=const offset_of!(Frame,simd),irq=sym irq,svc=sym svc,fail=sym fail);

#[inline(always)]
pub fn counter() -> u64 {
    let n;
    unsafe { asm!("isb","mrs {}, cntpct_el0",out(reg)n,options(nostack,preserves_flags)) };
    n
}
fn compare(n: u64) {
    unsafe {
        asm!("msr cntp_cval_el0, {}","msr cntp_ctl_el0, {}","isb",in(reg)n,in(reg)1u64,options(nostack,preserves_flags))
    }
}
fn disable() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    }
}
fn fail() -> ! {
    disable();
    uart_write(b"ORDINARY DEADLINE FAIL\n");
    park()
}
extern "C" fn idle() -> ! {
    loop {
        unsafe { asm!("wfi", options(nostack, preserves_flags)) }
    }
}
fn top(id: usize) -> usize {
    (&raw mut STACKS).addr() + (id + 1) * STACK_BYTES
}
fn initial(id: usize, pc: usize, arg: u64) -> *mut Frame {
    let f = (top(id) - size_of::<Frame>()) as *mut Frame;
    unsafe {
        (*f).elr = pc as u64;
        (*f).spsr = 0x345;
        (*f).gpr[0] = arg;
        (*f).gpr[30] = ordinary_deadline_return as *const () as u64;
    }
    f
}
fn number(n: i128) {
    let mut out = [0u8; 40];
    let mut i = out.len();
    let mut v = n.unsigned_abs();
    loop {
        i -= 1;
        out[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    if n < 0 {
        i -= 1;
        out[i] = b'-';
    }
    uart_write(&out[i..]);
}
fn line(label: &[u8], values: &[i128]) {
    uart_write(label);
    for &v in values {
        uart_write(b" ");
        number(v);
    }
    uart_write(b"\n");
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Row {
    event: [u64; 19],
    job: [[i128; 6]; N],
}
const EMPTY: Row = Row {
    event: [0; 19],
    job: [[0; 6]; N],
};
struct Runtime {
    jobs: &'static [Job],
    policy: DeadlineScheduler<N>,
    q: u64,
    s: u64,
    grid: u64,
    owner: usize,
    frames: [*mut Frame; N + 1],
    released: [bool; N],
    done: [bool; N],
    consumption: [u64; N],
    service: [u64; N],
    exhausted: [bool; N],
    len: usize,
}
impl Runtime {
    fn at(&self, k: u64) -> u64 {
        jobs::absolute(self.s, self.q, k).unwrap()
    }
    fn log(&self) {
        let rows = unsafe { &*(&raw const ordinary_deadline_rows) };
        for row in &rows[..self.len] {
            line(b"D", &row.event.map(i128::from));
            for j in &row.job {
                line(b"J", j);
            }
        }
    }
    fn stop(&self) -> ! {
        self.log();
        fail()
    }
    fn dispatch(
        &mut self,
        kind: u64,
        frame: *mut Frame,
        entry: u64,
        handler: u64,
        program: u64,
        iar: u64,
    ) -> *mut Frame {
        let k = (entry - self.s) / self.q;
        let out = self.owner;
        let previous = unsafe { ptr::read_volatile(&raw const deadline_restore_count) };
        let elr = if frame.is_null() {
            0
        } else {
            unsafe { (*frame).elr }
        };
        if !frame.is_null() {
            if frame.addr() < top(out) - STACK_BYTES
                || frame.addr() + size_of::<Frame>() > top(out)
                || entry < previous
            {
                self.stop()
            }
            self.frames[out] = frame;
        }
        let mut newly_exhausted = 0;
        if out < N {
            self.service[out] += entry - previous;
            self.consumption[out] += k - self.policy.now();
            let estimate = self.jobs.iter().find(|j| j.id == out).unwrap().cost;
            if self.consumption[out] >= estimate && !self.exhausted[out] {
                self.exhausted[out] = true;
                newly_exhausted = 1 << out;
            }
        }
        self.policy.account(k).unwrap();
        let mut missed = 0;
        if kind == 2 {
            if out == IDLE
                || self.done[out]
                || elr != (&raw const ordinary_deadline_after_svc).addr() as u64
            {
                self.stop()
            }
            self.policy.complete(k, out).unwrap();
            self.done[out] = true;
            missed = u64::from(
                entry > self.at(self.jobs.iter().find(|j| j.id == out).unwrap().deadline),
            );
        }
        let mut releases = 0;
        for j in self.jobs {
            if !self.released[j.id] && j.release <= k {
                self.policy.release(k, j.id, j.cost, j.deadline).unwrap();
                self.released[j.id] = true;
                releases |= 1 << j.id;
            }
        }
        let mut queue = 0;
        for &id in self.policy.ready() {
            queue = queue * 10 + id as u64 + 1;
        }
        self.owner = self.policy.decide(k).unwrap().unwrap_or(IDLE);
        let next = self.frames[self.owner];
        let mut row = EMPTY;
        for id in 0..N {
            row.job[id] = [
                id as i128,
                self.policy.remaining(id).map_or(-1, i128::from),
                self.policy.laxity(id).unwrap_or(i128::MIN),
                self.consumption[id] as i128,
                self.service[id] as i128,
                jobs::PROGRESS[id].load(Ordering::Relaxed) as i128,
            ];
        }
        if self.len == CAP {
            self.stop()
        }
        row.event = [
            kind,
            k,
            self.grid,
            self.at(self.grid),
            program,
            iar,
            previous,
            entry,
            handler,
            out as u64,
            self.owner as u64,
            frame.addr() as u64,
            elr,
            next.addr() as u64,
            unsafe { (*next).elr },
            queue,
            releases,
            newly_exhausted,
            missed,
        ];
        unsafe {
            ptr::write_volatile(
                (&raw mut ordinary_deadline_rows)
                    .cast::<Row>()
                    .add(self.len),
                row,
            )
        };
        self.len += 1;
        unsafe { ptr::write_volatile(&raw mut ordinary_deadline_row_count, self.len as u64) };
        if k > HORIZON
            || (kind != 2 && (self.grid != k || self.at(k) != program))
            || (kind == 2 && k >= self.grid)
        {
            self.stop()
        }
        if self.jobs.iter().all(|j| self.done[j.id]) {
            disable();
            if cfg!(jnox_deadline_trace) {
                self.log();
            }
            uart_write(b"ORDINARY DEADLINE FINISHED\n");
            park()
        }
        if kind != 2 {
            self.grid += 1;
            compare(self.at(self.grid));
        }
        next
    }
}
extern "C" fn irq(f: *mut Frame) -> *mut Frame {
    let handler = counter();
    let iar = gic_read(GICC + 0x0c);
    let program: u64;
    let ctl: u64;
    unsafe {
        asm!("mrs {}, cntp_cval_el0","mrs {}, cntp_ctl_el0",out(reg)program,out(reg)ctl,options(nostack,preserves_flags))
    };
    if iar & 0x3ff != TIMER_IRQ || ctl & 7 != 5 {
        fail()
    }
    disable();
    gic_write(GICC + 0x10, iar);
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
        (*(&raw mut STATE)).as_mut().unwrap().dispatch(
            1,
            f,
            (*f).reserved,
            handler,
            program,
            iar as u64,
        )
    }
}
extern "C" fn svc(f: *mut Frame) -> *mut Frame {
    let handler = counter();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1",out(reg)esr,options(nostack,preserves_flags)) };
    if esr >> 26 != 0x15 || (esr & 0x01ff_ffff) != 1 {
        fail()
    }
    unsafe {
        (*(&raw mut STATE))
            .as_mut()
            .unwrap()
            .dispatch(2, f, (*f).reserved, handler, 0, 0)
    }
}
pub fn run() -> ! {
    init_timer_irq();
    let hz: u64;
    unsafe { asm!("mrs {}, cntfrq_el0",out(reg)hz,options(nostack,preserves_flags)) };
    let llf = cfg!(any(
        jnox_app = "ordinary-llf",
        jnox_app = "ordinary-llf-overload"
    ));
    let overload = cfg!(jnox_app = "ordinary-llf-overload");
    let policy = if llf {
        DeadlinePolicy::Llf
    } else {
        DeadlinePolicy::Edf
    };
    let jobs: &[Job] = if overload {
        &jobs::LLF
    } else if llf {
        &jobs::LLF[..2]
    } else {
        &jobs::EDF
    };
    let q = hz / 10;
    let s = counter().checked_add(q).unwrap();
    let decision = jobs::admit(policy, jobs, hz, s);
    let refused = match decision {
        Ok(()) => 0,
        Err(e) => e as u64,
    };
    line(
        b"CONFIG",
        &[
            hz as i128,
            s as i128,
            q as i128,
            llf as i128,
            overload as i128,
            refused as i128,
            HORIZON as i128,
            size_of::<Frame>() as i128,
        ],
    );
    if overload {
        if refused != jobs::Refusal::Miss as u64 {
            fail()
        }
    } else if refused != 0 {
        fail()
    }
    let mut frames = [ptr::null_mut(); N + 1];
    frames[IDLE] = initial(IDLE, idle as *const () as usize, 0);
    for j in jobs {
        let pc = j.entry.unwrap() as usize;
        frames[j.id] = initial(j.id, pc, hz * j.milliseconds / 1000);
        line(
            b"JOB",
            &[
                j.id as i128,
                j.release as i128,
                j.deadline as i128,
                j.cost as i128,
                j.milliseconds as i128,
                pc as i128,
                top(j.id) as i128,
            ],
        );
    }
    unsafe {
        STATE = Some(Runtime {
            jobs,
            policy: DeadlineScheduler::new(policy, 0, 1).unwrap(),
            q,
            s,
            grid: 0,
            owner: IDLE,
            frames,
            released: [false; N],
            done: [false; N],
            consumption: [0; N],
            service: [0; N],
            exhausted: [false; N],
            len: 0,
        });
        if counter() >= s {
            fail()
        }
        compare(s);
        sched_start_frame(frames[IDLE])
    }
}
