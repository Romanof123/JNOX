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

#[cfg(jnox_deadline_recurring)]
#[path = "deadline_recurring.rs"]
mod recurring;

const IDLE: usize = N;
const SERVICE: usize = N + 1;
const SUPPLY_PROFILE: bool = cfg!(any(
    jnox_deadline_recurring,
    jnox_app = "ordinary-edf-supply",
    jnox_app = "ordinary-llf-supply",
    jnox_app = "ordinary-edf-supply-refuse",
    jnox_app = "ordinary-llf-supply-refuse"
));
const CONTEXTS: usize = N + 1 + SUPPLY_PROFILE as usize;
static SERVICE_SUM: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static SERVICE_SP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
extern "C" fn supply_service() -> ! {
    let sp: u64;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nostack, preserves_flags)) }
    SERVICE_SP.store(sp, Ordering::Relaxed);
    let mut sum = 0;
    for sample in 1u64..=64 {
        let value = core::hint::black_box(sample);
        sum += value * value;
    }
    SERVICE_SUM.store(sum, Ordering::Relaxed);
    idle()
}
const CAP: usize = 24;
static mut STACKS: [Stack; CONTEXTS] = [const { Stack([0; STACK_BYTES]) }; CONTEXTS];
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
    #[cfg(jnox_deadline_recurring)]
    recurring::deadline_recurring_launch[1].fetch_add(1, Ordering::Relaxed);
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
    #[cfg(jnox_deadline_recurring)]
    recurring::deadline_recurring_launch[0].fetch_add(1, Ordering::Relaxed);
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
    #[cfg(jnox_deadline_recurring)]
    reserved: u64,
    jobs: &'static [Job],
    policy: DeadlineScheduler<N>,
    q: u64,
    s: u64,
    grid: u64,
    owner: usize,
    frames: [*mut Frame; CONTEXTS],
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
    #[cfg(jnox_deadline_owner_cap)]
    fn score_owner_windows(&self) {
        let rows = unsafe { &*(&raw const ordinary_deadline_rows) };
        let rows = &rows[..self.len];
        let mut peak_events = 0;
        let mut peak_changes = 0;
        let mut total_changes = 0;
        let mut first = 0;
        let mut changes = 0;
        for (i, row) in rows.iter().enumerate() {
            let e = &row.event;
            let returns = i + 1 < rows.len();
            let changed = returns && e[9] != e[10];
            if returns {
                let following = &rows[i + 1].event;
                if following[9] != e[10] || following[6] < e[8] || following[6] > following[7] {
                    self.stop()
                }
            }
            changes += usize::from(changed);
            total_changes += usize::from(changed);
            // Any set of entries with last-first < Q fits in a full window
            // inside the observed domain, including at either domain edge.
            while e[7] - rows[first].event[7] >= self.q {
                let old = &rows[first].event;
                changes -= usize::from(old[9] != old[10]);
                first += 1;
            }
            let events = i + 1 - first;
            peak_events = peak_events.max(events);
            peak_changes = peak_changes.max(changes);
            line(b"OWNER_EVENT", &[i as i128, e[0] as i128, e[7] as i128,
                e[9] as i128, e[10] as i128, returns as i128, changed as i128,
                events as i128, changes as i128]);
        }
        let bound = jobs::owner_window_bound(self.q, HORIZON + 1, self.q, self.jobs.len() as u64).unwrap();
        line(b"OWNER_PEAK", &[rows.len() as i128, total_changes as i128, peak_events as i128,
            peak_changes as i128, bound as i128]);
        if peak_events as u64 > bound || peak_changes as u64 > bound {
            fail()
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
            if out >= N
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
        #[cfg(jnox_deadline_recurring)]
        let mask = self.reserved;
        #[cfg(not(jnox_deadline_recurring))]
        let mask = jobs::RESERVED;
        let withheld = SUPPLY_PROFILE && k < HORIZON && mask & (1 << k) != 0;
        #[cfg(jnox_deadline_recurring)]
        recurring::boundary(self, k, kind, out);
        if withheld {
            self.policy.withhold(k).unwrap();
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
        self.owner = if withheld {
            SERVICE
        } else {
            self.policy.decide(k).unwrap().unwrap_or(IDLE)
        };
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
        #[cfg(jnox_deadline_owner_cap)]
        if missed != 0 || handler < entry
            || self.jobs.iter().any(|j| !self.done[j.id] && entry >= self.at(j.deadline)) {
            self.stop()
        }
        if self.jobs.iter().all(|j| self.done[j.id]) {
            disable();
            if cfg!(any(jnox_deadline_trace, jnox_deadline_owner_cap)) {
                self.log();
            }
            #[cfg(jnox_deadline_owner_cap)]
            self.score_owner_windows();
            #[cfg(jnox_deadline_recurring)]
            recurring::finish(self);
            #[cfg(not(jnox_deadline_recurring))]
            if SUPPLY_PROFILE {
                let rows = unsafe { &*(&raw const ordinary_deadline_rows) };
                let progress_at = |out, owner| {
                    rows[..self.len]
                        .iter()
                        .find(|row| {
                            row.event[0] == 1 && row.event[9] == out && row.event[10] == owner
                        })
                        .map(|row| row.job[0][5])
                };
                let before = progress_at(0, SERVICE as u64);
                let resumed = progress_at(SERVICE as u64, 0);
                let finished = jobs::PROGRESS[0].load(Ordering::Relaxed) as i128;
                line(
                    b"A_PROGRESS",
                    &[before.unwrap_or(-1), resumed.unwrap_or(-1), finished],
                );
                if before.is_none_or(|n| n <= 0 || resumed != Some(n) || finished <= n) {
                    fail()
                }
                line(
                    b"RESULT",
                    &[
                        SERVICE_SUM.load(Ordering::Relaxed) as i128,
                        SERVICE_SP.load(Ordering::Relaxed) as i128,
                        jobs::PRODUCT[0].load(Ordering::Relaxed) as i128,
                        jobs::PRODUCT[1].load(Ordering::Relaxed) as i128,
                    ],
                );
            }
            uart_write(b"ORDINARY DEADLINE FINISHED\n");
            #[cfg(jnox_deadline_recurring)]
            recurring::deadline_recurring_terminal();
            #[cfg(not(jnox_deadline_recurring))]
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
        jnox_app = "ordinary-llf-overload",
        jnox_app = "ordinary-llf-recurring",
        jnox_app = "ordinary-llf-recurring-refuse",
        jnox_app = "ordinary-llf-supply",
        jnox_app = "ordinary-llf-supply-refuse"
    ));
    let overload = cfg!(jnox_app = "ordinary-llf-overload");
    let policy = if llf {
        DeadlinePolicy::Llf
    } else {
        DeadlinePolicy::Edf
    };
    let tight = cfg!(any(
        jnox_app = "ordinary-edf-recurring-refuse",
        jnox_app = "ordinary-llf-recurring-refuse",
        jnox_app = "ordinary-edf-supply-refuse",
        jnox_app = "ordinary-llf-supply-refuse"
    ));
    #[cfg(jnox_deadline_recurring)]
    let jobs: &[Job] = if tight { &jobs::RECURRING_TIGHT } else { &jobs::RECURRING };
    #[cfg(not(jnox_deadline_recurring))]
    let jobs: &[Job] = if tight {
        &jobs::TIGHT
    } else if SUPPLY_PROFILE {
        &jobs::SUPPLIED
    } else if overload {
        &jobs::LLF
    } else if llf {
        &jobs::LLF[..2]
    } else {
        &jobs::EDF
    };
    let q = hz / 10;
    let s = counter().checked_add(q).unwrap();
    let full = jobs::admit(policy, jobs, hz, s);
    #[cfg(jnox_deadline_owner_cap)]
    let full_ok = full.is_ok();
    #[cfg(jnox_deadline_recurring)]
    let (mask, service_entry) = {
        let mask = jobs::recurring_mask(recurring::OFFSET, recurring::PERIOD, recurring::WIDTH).unwrap();
        let single_ok = jobs::admit_with_supply(policy, jobs, hz, s, 1 << recurring::OFFSET).is_ok();
        line(b"CALENDAR", &[recurring::OFFSET.into(), recurring::PERIOD.into(),
            recurring::WIDTH.into(), mask.into(), i128::from(single_ok)]);
        for j in jobs {
            line(b"RECURRING_JOB", &[j.id as i128, j.release.into(), j.deadline.into(), j.cost.into(),
                j.milliseconds.into()]);
        }
        recurring::prelaunch();
        if !full_ok || !single_ok { fail() }
        (mask, recurring::activate as *const () as usize)
    };
    #[cfg(not(jnox_deadline_recurring))]
    let (mask, service_entry) = (jobs::RESERVED, supply_service as *const () as usize);
    let decision = if SUPPLY_PROFILE {
        line(
            b"SUPPLY",
            &[
                mask as i128,
                full.is_ok() as i128,
                tight as i128,
                top(SERVICE) as i128,
                STACK_BYTES as i128,
                service_entry as i128,
            ],
        );
        jobs::admit_with_supply(policy, jobs, hz, s, mask)
    } else {
        full
    };
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
    if tight {
        let expected = if llf {
            jobs::Refusal::Miss
        } else {
            jobs::Refusal::Demand
        };
        if refused != expected as u64 {
            fail()
        }
        uart_write(b"ORDINARY DEADLINE REFUSED BEFORE LAUNCH\n");
        #[cfg(jnox_deadline_recurring)]
        recurring::deadline_recurring_terminal();
        #[cfg(not(jnox_deadline_recurring))]
        park()
    }
    if overload {
        if refused != jobs::Refusal::Miss as u64 {
            fail()
        }
    } else if refused != 0 {
        fail()
    }
    #[cfg(jnox_deadline_owner_cap)]
    {
        let ceiling: u64 = env!("JNOX_OWNER_CEILING").parse().unwrap();
        if !full_ok {
            fail()
        }
        let bound = jobs::owner_window_bound(q, HORIZON + 1, q, jobs.len() as u64).unwrap();
        let admitted = ceiling >= bound;
        line(b"OWNER_CAP", &[s as i128, jobs::absolute(s, q, HORIZON + 1).unwrap() as i128,
            q as i128, jobs.len() as i128, bound as i128, ceiling as i128, admitted as i128]);
        line(b"OWNER_PRELAUNCH", &[jobs::PROGRESS[0].load(Ordering::Relaxed) as i128,
            jobs::PROGRESS[1].load(Ordering::Relaxed) as i128,
            SERVICE_SUM.load(Ordering::Relaxed) as i128,
            unsafe { ptr::read_volatile(&raw const ordinary_deadline_row_count) } as i128]);
        if !admitted {
            uart_write(b"ORDINARY OWNER CAP REFUSED BEFORE LAUNCH\n");
            #[cfg(jnox_deadline_recurring)]
            recurring::deadline_recurring_terminal();
            #[cfg(not(jnox_deadline_recurring))]
            park()
        }
    }
    let mut frames = [ptr::null_mut(); CONTEXTS];
    #[cfg(not(jnox_deadline_recurring))]
    if SUPPLY_PROFILE {
        frames[SERVICE] = initial(SERVICE, supply_service as *const () as usize, 0);
    }
    frames[IDLE] = initial(IDLE, idle as *const () as usize, 0);
    for j in jobs {
        let pc = j.entry.unwrap() as usize;
        let duration = hz.checked_mul(j.milliseconds).unwrap() / 1000;
        let arg = if SUPPLY_PROFILE && j.id == 0 {
            let stop = jobs::absolute(s, 1, duration).unwrap();
            line(b"A_STOP", &[stop as i128]);
            stop
        } else {
            duration
        };
        frames[j.id] = initial(j.id, pc, arg);
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
            #[cfg(jnox_deadline_recurring)]
            reserved: mask,
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
        #[cfg(jnox_deadline_recurring)]
        recurring::deadline_recurring_launch[2].fetch_add(1, Ordering::Relaxed);
        sched_start_frame(frames[IDLE])
    }
}
