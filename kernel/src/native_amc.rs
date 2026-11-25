//! One CPU, finite non-overlapping release tokens and actual EL0 occupancy.
use super::*;
#[cfg(jnox_native_amc_clock_page)]
#[path = "native_amc_time.rs"]
mod clock_page;
use crate::domain_policy::{CriticalityDomain, CriticalityMode, ExecutionBudget};
const IDLE_PID: usize = COUNT;
const TERMINAL: usize = COUNT + 1;
const NA: u64 = u64::MAX;
const MAX_ROWS: usize = 8192;
const WIDTH: usize = 23;
#[unsafe(no_mangle)]
static mut native_amc_intervals: [[u64; WIDTH]; MAX_ROWS] = [[0; WIDTH]; MAX_ROWS];
#[unsafe(no_mangle)]
static mut native_amc_results: [[[u8; 1024]; 16]; COUNT] = [[[0; 1024]; 16]; COUNT];
#[unsafe(no_mangle)]
static mut native_server_compare: u64 = NA;
#[unsafe(no_mangle)]
static mut native_server_last_return: u64 = 0;
#[unsafe(no_mangle)]
static mut native_amc_remaining: u64 = 0;
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = ptr::null_mut();
const CLOCK_ARCHIVE_BYTES: usize = 1 << 20;
static mut CLOCK_ARCHIVE: [u8; CLOCK_ARCHIVE_BYTES] = [0; CLOCK_ARCHIVE_BYTES];
static mut CLOCK_INDEX: [[[u64; 4]; 16]; COUNT] = [[[0; 4]; 16]; COUNT];
static mut LOG: [[u64; 10]; 640] = [[0; 10]; 640];
#[derive(Clone, Copy, Default)]
struct Job {
    live: bool,
    gen_id: u64,
    used: u64,
    credited: u64,
    due: u64,
    miss: bool,
    wake: Option<u64>,
}
struct Runtime {
    hz: u64,
    epoch: u64,
    q: u64,
    domain: CriticalityDomain<COUNT>,
    images: [&'static [u8]; COUNT],
    witness: [Option<(usize, usize)>; COUNT],
    clock_evidence: [Option<(usize, usize)>; COUNT],
    clock_bytes: usize,
    clock_lost: u64,
    jobs: [Job; COUNT],
    attempt: [usize; COUNT],
    accepted: [usize; COUNT],
    reset: usize,
    owner: usize,
    rows: usize,
    lost: u64,
    logs: usize,
    log_lost: u64,
    landing: u64,
}
static mut STATE: Option<Runtime> = None;
fn state() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(27)) }
}
fn plus(a: u64, b: u64) -> u64 {
    a.checked_add(b).unwrap_or_else(|| fail(27))
}
fn times(a: u64, b: u64) -> u64 {
    a.checked_mul(b).unwrap_or_else(|| fail(27))
}
fn idle_top() -> *mut u8 {
    (&raw mut IDLE)
        .cast::<u8>()
        .wrapping_add(context::STACK_BYTES)
}
fn root() -> u64 {
    let v;
    unsafe {
        asm!("mrs {},ttbr0_el1",out(reg)v,options(nostack,preserves_flags));
    }
    v
}
fn symbol(image: &[u8], plan: &LoadPlan<'_>, name: &[u8], maximum: u64) -> Option<(usize, usize)> {
    let off = usize::try_from(u64_at(image, 40)?).ok()?;
    let n = u16::from_le_bytes(image.get(60..62)?.try_into().ok()?) as usize;
    let headers = image.get(off..off.checked_add(n.checked_mul(64)?)?)?;
    let mut found = None;
    for h in headers.chunks_exact(64).filter(|h| u32_at(h, 4) == Some(2)) {
        if u64_at(h, 56)? != 24 {
            return None;
        }
        let i = u32_at(h, 40)? as usize;
        let strh = headers.chunks_exact(64).nth(i)?;
        if u32_at(strh, 4)? != 3 {
            return None;
        }
        let names = section_bytes(image, strh)?;
        for sym in section_bytes(image, h)?.chunks_exact(24) {
            if sym[4] & 15 != 1 || !names.get(u32_at(sym, 0)? as usize..)?.starts_with(name) {
                continue;
            }
            let addr = u64_at(sym, 8)?;
            let size = u64_at(sym, 16)?;
            if found.is_some()
                || !(1..=maximum).contains(&size)
                || !plan.segments().iter().any(|s| {
                    s.permission == Permission::ReadWrite
                        && addr >= s.address
                        && addr
                            .checked_add(size)
                            .zip(s.address.checked_add(s.memory_size))
                            .is_some_and(|(a, b)| a <= b)
                })
            {
                return None;
            }
            found = Some((addr as usize, size as usize));
        }
    }
    found
}
impl Runtime {
    fn hi(&self) -> bool {
        self.domain.mode() == CriticalityMode::High
    }
    fn log(&mut self, kind: u64, stamp: u64, id: usize, g: u64, values: [u64; 5]) {
        if self.logs == 640 {
            self.log_lost = self.log_lost.saturating_add(1);
            return;
        }
        unsafe {
            LOG[self.logs] = [
                kind,
                stamp,
                id as u64,
                g,
                self.hi() as u64,
                values[0],
                values[1],
                values[2],
                values[3],
                values[4],
            ];
        }
        self.logs += 1;
    }
    fn release_time(&self, i: usize) -> Option<u64> {
        let (_, _, _, _, _, p, off, count) = NATIVE_AMC_TASKS[i];
        let a = self.attempt[i] as u64;
        (a < count).then(|| plus(self.epoch, times(self.q, plus(off, times(a, p)))))
    }
    fn reset_time(&self) -> Option<u64> {
        NATIVE_AMC_RESETS
            .get(self.reset)
            .map(|v| plus(self.epoch, times(v.0, self.q)))
    }
    fn calendar(&self) -> u64 {
        let mut next = self.reset_time().unwrap_or(NA);
        for i in 0..COUNT {
            if let Some(t) = self.release_time(i) {
                next = next.min(t);
            }
            let j = self.jobs[i];
            if j.live {
                if !j.miss {
                    next = next.min(j.due);
                }
                if let Some(t) = j.wake {
                    next = next.min(t);
                }
            }
        }
        next
    }
    fn retire(&mut self, i: usize, kind: u64, at: u64, status: u64) {
        let j = self.jobs[i];
        if !j.live {
            fail(27);
        }
        if let Some((addr, len)) = self.witness[i] {
            let dst =
                unsafe { &mut (&mut *(&raw mut native_amc_results[i][j.gen_id as usize]))[..len] };
            if !native_el0_mmu::copy_from_process(i, addr, dst) {
                fail(27);
            }
        }
        if let Some((addr, len)) = self.clock_evidence[i] {
            let offset = self.clock_bytes;
            let stored = if len <= CLOCK_ARCHIVE_BYTES - offset {
                let dst = unsafe { &mut (&mut *(&raw mut CLOCK_ARCHIVE))[offset..offset + len] };
                if !native_el0_mmu::copy_from_process(i, addr, dst) {
                    fail(27);
                }
                self.clock_bytes += len;
                len
            } else {
                self.clock_lost = self.clock_lost.saturating_add(1);
                0
            };
            unsafe {
                CLOCK_INDEX[i][j.gen_id as usize] =
                    [addr as u64, len as u64, offset as u64, stored as u64];
            }
        }
        if kind != 5 && !self.domain.finish(i) {
            fail(27);
        }
        #[cfg(jnox_native_amc_clock_page)]
        clock_page::revoke(i);
        self.jobs[i].live = false;
        self.jobs[i].wake = None;
        self.log(
            kind,
            at,
            i,
            j.gen_id,
            [
                j.used,
                j.due,
                status,
                j.wake.unwrap_or(NA),
                self.witness[i].map_or(0, |p| p.1 as u64),
            ],
        );
    }
    fn admit(&mut self, i: usize, at: u64, scheduled: u64) {
        let a = self.attempt[i];
        self.attempt[i] += 1;
        let due = plus(scheduled, times(NATIVE_AMC_TASKS[i].4, self.q));
        let refusal = if self.jobs[i].live {
            1
        } else if self.hi() && !NATIVE_AMC_TASKS[i].0 {
            2
        } else if at >= due {
            3
        } else {
            0
        };
        if refusal != 0 {
            self.log(
                2,
                at,
                i,
                self.accepted[i] as u64,
                [scheduled, due, a as u64, refusal, 0],
            );
            return;
        }
        if !self.domain.release(i) {
            fail(27);
        }
        let g = self.accepted[i];
        self.accepted[i] += 1;
        if a != 0 {
            let plan = validated_image(self.images[i]);
            let buf = unsafe { &mut *(&raw mut STARTUP_TAILS[i]) };
            let l = native_stack::build(NATIVE_AMC_ARGV[i][a], false, STACK_BOTTOM, STACK_TOP, buf)
                .unwrap_or_else(|| fail(27));
            unsafe {
                native_el0_mmu::amc_reload(i, &plan, &buf[..l.len]).unwrap_or_else(|_| fail(27));
                FRAMES[i] = EMPTY_FRAME;
                FRAMES[i].elr = plan.entry;
                FRAMES[i].sp_el0 = l.sp as u64;
            }
        }
        #[cfg(jnox_native_amc_clock_page)]
        clock_page::begin_generation(i, g as u64, self.hz);
        self.jobs[i] = Job {
            live: true,
            gen_id: g as u64,
            due,
            ..Job::default()
        };
        self.log(1, at, i, g as u64, [scheduled, due, a as u64, 0, 0]);
    }
    fn calendar_until(&mut self, now: u64) {
        loop {
            let t = self.calendar();
            if t == NA || t > now {
                break;
            }
            for i in 0..COUNT {
                let j = self.jobs[i];
                if !j.live {
                    continue;
                }
                if !j.miss && j.due == t {
                    self.jobs[i].miss = true;
                    self.log(7, now, i, j.gen_id, [t, j.used, 0, 0, 0]);
                }
                if j.wake == Some(t) {
                    self.jobs[i].wake = None;
                    self.log(8, now, i, j.gen_id, [t, j.used, 0, 0, 0]);
                }
            }
            if self.reset_time() == Some(t) {
                let approved = NATIVE_AMC_RESETS[self.reset].1;
                self.reset += 1;
                let ok = self.domain.reset(approved);
                self.log(6, now, IDLE_PID, NA, [t, approved as u64, ok as u64, 0, 0]);
            }
            for i in 0..COUNT {
                if self.release_time(i) == Some(t) {
                    self.admit(i, now, t);
                }
            }
        }
    }
    fn bounds(&self, i: usize) -> (u64, u64) {
        let (hi, _, lo, cap, ..) = NATIVE_AMC_TASKS[i];
        (times(self.q, lo), times(self.q, if hi { cap } else { lo }))
    }
    fn credit(&mut self, i: usize, end: u64) {
        let j = &mut self.jobs[i];
        if end > j.credited {
            let want = end - j.credited;
            if self.domain.grant(i, want) != Some(want) {
                fail(27);
            }
            j.credited = end;
        }
    }
    fn continuation(&mut self, i: usize, at: u64) {
        let j = self.jobs[i];
        if !j.live {
            return;
        }
        let (lo, hi) = self.bounds(i);
        if j.used >= hi {
            self.retire(i, 4, at, NA);
            return;
        }
        if !self.hi() && NATIVE_AMC_TASKS[i].0 && j.used >= lo && j.wake.is_none() {
            self.credit(i, plus(j.used, 1));
            if !self.hi() {
                fail(27);
            }
            self.log(9, counter(), i, j.gen_id, [lo, hi, j.used, 0, 0]);
            for k in 0..COUNT {
                if self.jobs[k].live && !NATIVE_AMC_TASKS[k].0 {
                    self.retire(k, 5, at, NA);
                }
            }
        }
    }
    fn pick(&self) -> usize {
        (0..COUNT)
            .filter(|&i| self.jobs[i].live && self.jobs[i].wake.is_none())
            .max_by_key(|&i| (NATIVE_AMC_TASKS[i].1, core::cmp::Reverse(i)))
            .unwrap_or_else(|| {
                if self.calendar() == NA {
                    TERMINAL
                } else {
                    IDLE_PID
                }
            })
    }
}
pub(super) fn start(hz: u64, images: [&'static [u8]; COUNT]) -> usize {
    let q = hz / 20;
    if q == 0 {
        fail(27);
    }
    let epoch = plus(counter(), q);
    for (i, &(hi, pr, lo, h, d, t, off, n)) in NATIVE_AMC_TASKS.iter().enumerate() {
        if pr == 0
            || lo == 0
            || lo > h
            || h > d
            || d > t
            || (!hi && lo != h)
            || !(1..=16).contains(&n)
            || NATIVE_AMC_ARGV[i].len() != n as usize
        {
            fail(27);
        }
        let end = plus(epoch, times(q, plus(plus(off, times(n - 1, t)), d)));
        if end == NA || end - epoch > i64::MAX as u64 {
            fail(27);
        }
    }
    if NATIVE_AMC_RESETS.len() > 16 || NATIVE_AMC_RESETS.windows(2).any(|v| v[0].0 >= v[1].0) {
        fail(27);
    }
    for &(k, _) in NATIVE_AMC_RESETS {
        if times(k, q) > i64::MAX as u64 || plus(epoch, times(k, q)) == NA {
            fail(27);
        }
    }
    let budgets = core::array::from_fn(|i| {
        let (hi, _, lo, h, ..) = NATIVE_AMC_TASKS[i];
        if hi {
            ExecutionBudget::High {
                lo: times(lo, q),
                hi: times(h, q),
            }
        } else {
            ExecutionBudget::Low(times(lo, q))
        }
    });
    #[cfg(jnox_native_amc_clock_page)]
    clock_page::enable(hz);
    let descriptors = images.map(|img| {
        let plan = validated_image(img);
        (
            symbol(img, &plan, b"amc_product\0", 1024),
            symbol(img, &plan, b"amc_clock_evidence\0", 73760),
        )
    });
    unsafe {
        STATE = Some(Runtime {
            hz,
            q,
            epoch,
            domain: CriticalityDomain::new(budgets).unwrap_or_else(|| fail(27)),
            images,
            witness: descriptors.map(|d| d.0),
            clock_evidence: descriptors.map(|d| d.1),
            clock_bytes: 0,
            clock_lost: 0,
            jobs: [Job::default(); COUNT],
            attempt: [0; COUNT],
            accepted: [0; COUNT],
            reset: 0,
            owner: IDLE_PID,
            rows: 0,
            lost: 0,
            logs: 0,
            log_lost: 0,
            landing: 0,
        });
        asm!("msr vbar_el1, {}","isb",in(reg)&raw const native_el0_vectors,options(nostack));
        arm_compare(Some(state().calendar()));
        native_sleep_switch(idle_top());
    }
    state().owner
}

enum Input {
    Timer(u32),
    Call,
    Gate,
}
fn decide(entry: u64, frame: Option<&mut NativeFrame>, input: Input) -> usize {
    let r = state();
    let src = r.owner;
    #[cfg(jnox_native_amc_clock_page)]
    clock_page::enter(src);
    if frame.is_some() != (src < COUNT) {
        fail(27);
    }
    let cval: u64;
    unsafe {
        asm!("mrs {},cntp_cval_el0",out(reg)cval,options(nostack,preserves_flags));
    }
    let start = unsafe { native_server_last_return };
    unsafe {
        native_server_last_return = 0;
    }
    let prior_root = root();
    let mut src_pc = 0;
    let mut src_sp = 0;
    let mut gen_id = NA;
    let mut delta = 0;
    let mut used = 0;
    let mut op = match input {
        Input::Timer(iar) => u64::from(iar),
        _ => 0,
    };
    let mut arg = 0;
    let mut result = 0;
    let mut spill = 0;
    let cause = match input {
        Input::Timer(_) => 1,
        Input::Call => 2,
        Input::Gate => 3,
    };
    if let Some(f) = frame {
        if unsafe { CURRENT } != src || !r.jobs[src].live {
            fail(27);
        }
        src_pc = f.elr;
        src_sp = f.sp_el0;
        gen_id = r.jobs[src].gen_id;
        let (lo, hi) = r.bounds(src);
        let limit = if r.hi() { hi } else { lo };
        if cause != 3 {
            if start == 0 {
                fail(27);
            }
            delta = entry.checked_sub(start).unwrap_or_else(|| fail(27));
            r.jobs[src].used = plus(r.jobs[src].used, delta);
        }
        used = r.jobs[src].used;
        spill = used.saturating_sub(limit);
        r.credit(src, used.min(limit));
        if cause == 2 {
            op = f.x[8];
            arg = f.x[0];
            result = match op {
                1 => arg,
                6 => {
                    (if arg == 2 {
                        write_clock(f.x[1] as usize, used, r.hz)
                    } else {
                        clock_gettime(arg, f.x[1] as usize, entry)
                    }) as u64
                }
                7 => match sleep_until(f) {
                    Err(e) => e as u64,
                    Ok(t) if t.saturating_sub(entry) > i64::MAX as u64 => (-75i64) as u64,
                    Ok(t) => {
                        if t > entry {
                            r.jobs[src].wake = Some(t);
                        }
                        0
                    }
                },
                8 => 0,
                #[cfg(jnox_native_amc_clock_page)]
                20 => clock_page::VA,
                9..=18 => (-95i64) as u64,
                _ => (-38i64) as u64,
            };
            f.x[0] = result;
        }
        unsafe {
            FRAMES[src] = *f;
        }
        if cause == 2 && op == 1 {
            let j = r.jobs[src];
            if entry > j.due && !j.miss {
                r.jobs[src].miss = true;
                r.log(7, entry, src, gen_id, [j.due, used, 0, 0, 0]);
            }
            r.retire(src, if used <= limit { 3 } else { 4 }, entry, arg);
        } else {
            r.continuation(src, entry);
        }
    } else if cause == 1 {
        unsafe {
            asm!("mrs {},elr_el1",out(reg)src_pc,options(nostack,preserves_flags));
        }
        src_sp = idle_top() as u64;
    }
    let at = counter();
    r.calendar_until(at);
    let mut dst = r.pick();
    if dst < COUNT {
        r.continuation(dst, at);
        dst = r.pick();
    }
    let next_cal = r.calendar();
    let mut left = 0;
    let mut dst_pc = 0;
    let mut dst_sp = 0;
    let mut dst_gen = NA;
    if dst < COUNT {
        let (lo, hi) = r.bounds(dst);
        left = (if r.hi() { hi } else { lo })
            .checked_sub(r.jobs[dst].used)
            .filter(|&v| v > 0)
            .unwrap_or_else(|| fail(27));
        dst_gen = r.jobs[dst].gen_id;
        unsafe {
            dst_pc = FRAMES[dst].elr;
            dst_sp = FRAMES[dst].sp_el0;
        }
    }
    unsafe {
        native_server_compare = if dst < COUNT { next_cal } else { NA };
        native_amc_remaining = left;
    }
    arm_compare(if dst == TERMINAL || next_cal == NA {
        None
    } else {
        Some(next_cal)
    });
    r.owner = dst;
    let idx = r.rows;
    #[cfg(jnox_native_amc_clock_page)]
    if dst < COUNT {
        clock_page::prepare(dst, dst_gen, r.jobs[dst].used, idx);
    }
    if idx < MAX_ROWS {
        unsafe {
            native_amc_intervals[idx] = [
                cause, op, entry, start, src as u64, gen_id, delta, used, spill, cval, src_pc,
                src_sp, prior_root, dst as u64, dst_gen, dst_pc, dst_sp, next_cal, left, arg,
                result, 0, 0,
            ];
        }
        r.rows += 1;
    } else {
        r.lost = r.lost.saturating_add(1);
    }
    dst
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    let r = state();
    if r.rows > 0 && r.lost == 0 {
        unsafe {
            native_amc_intervals[r.rows - 1][21] = root();
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    let r = state();
    r.landing = stamp;
    if r.rows > 0 && r.lost == 0 {
        unsafe {
            native_amc_intervals[r.rows - 1][22] = stamp;
        }
    }
}
fn restore(next: usize, f: &mut NativeFrame) {
    if next == TERMINAL {
        finish();
    }
    if next == IDLE_PID {
        unsafe {
            native_sleep_switch(idle_top());
        }
    }
    let dst = state().owner;
    if dst >= COUNT {
        fail(27);
    }
    unsafe {
        if CURRENT != dst {
            native_el0_mmu::switch(dst).unwrap_or_else(|_| fail(27));
            CURRENT = dst;
        }
        *f = FRAMES[dst];
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_server_reselect(at: u64, frame: *mut NativeFrame) -> u64 {
    let mut f = unsafe { frame.as_mut() };
    let next = decide(at, f.as_deref_mut(), Input::Gate);
    if let Some(f) = f {
        restore(next, f);
    }
    if next == TERMINAL {
        finish();
    }
    (next < COUNT) as u64
}
pub(super) fn irq(entry: u64, frame: *mut NativeFrame, iar: u32) -> u64 {
    let mut f = unsafe { frame.as_mut() };
    let next = decide(entry, f.as_deref_mut(), Input::Timer(iar));
    if iar & 1023 < 1020 {
        unsafe {
            asm!("dsb sy", options(nostack, preserves_flags));
        }
        context::gic_write(context::GICC + 0x10, iar);
        unsafe {
            asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
    if next == TERMINAL {
        finish();
    }
    if let Some(f) = f {
        restore(next, f);
    }
    (next < COUNT) as u64
}
pub(super) fn syscall(entry: u64, f: &mut NativeFrame) {
    let next = decide(entry, Some(f), Input::Call);
    restore(next, f);
}
fn print(tag: &[u8], vals: &[u64]) {
    uart_write(b"NATIVE AMC ");
    uart_write(tag);
    for &v in vals {
        uart_write(b" ");
        unsigned(v);
    }
    uart_write(b"\n");
}
fn finish() -> ! {
    let r = state();
    arm_compare(None);
    print(
        b"CONFIG",
        &[
            1,
            r.epoch,
            r.hz,
            r.q,
            COUNT as u64,
            IDLE_PID as u64,
            TERMINAL as u64,
            unsafe { CLOCK_DOMAIN.0 },
        ],
    );
    for i in 0..COUNT {
        let (hi, p, lo, h, d, t, off, n) = NATIVE_AMC_TASKS[i];
        let (addr, len) = r.witness[i].unwrap_or((0, 0));
        print(
            b"TASK",
            &[
                i as u64,
                hi as u64,
                p as u64,
                lo,
                h,
                d,
                t,
                off,
                n,
                addr as u64,
                len as u64,
            ],
        );
    }
    for i in 0..r.logs {
        print(b"LIFE", unsafe { &*(&raw const LOG[i]) });
    }
    // All switch and clock-call rows remain available in captured privileged RAM.
    // Avoid printing thousands of monitoring calls before the capture can stop.
    print(
        b"INTERVALS",
        &[
            (&raw const native_amc_intervals).addr() as u64,
            r.rows as u64,
            WIDTH as u64,
            r.lost,
        ],
    );
    print(
        b"RESULTS",
        &[(&raw const native_amc_results).addr() as u64, 16, 1024],
    );
    #[cfg(jnox_native_amc_clock_page)]
    clock_page::report(r.rows, r.lost);
    if r.clock_evidence.iter().any(Option::is_some) {
        print(
            b"CLOCK_EVIDENCE",
            &[
                1,
                (&raw const CLOCK_INDEX).addr() as u64,
                (&raw const CLOCK_ARCHIVE).addr() as u64,
                r.clock_bytes as u64,
                r.clock_lost,
            ],
        );
    }
    print(b"END", &[r.logs as u64, r.log_lost, r.landing]);
    head(10, 1);
    head(1, 3);
    park()
}
