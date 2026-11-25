//! Private native lottery and stride processes on the architectural physical timer.
use super::*;
#[path = "feedback_policy.rs"]
mod feedback_policy;
#[path = "proportional_policy.rs"]
mod policy;
use policy::{FirstRejection, NativeWindowInput, Proportional, Rejection};

const IDLE_OWNER: usize = 3;
const TERMINAL: usize = 4;
const LIMIT: usize = 4096;
const ALL: u32 = (1 << COUNT) - 1;
const LOTTERY: bool = matches!(NATIVE_POLICY.as_bytes(), b"lottery");
static mut EVENTS: [[u64; 40]; LIMIT] = [[0; 40]; LIMIT];
static mut DRAWS: [[u64; 6]; LIMIT] = [[0; 6]; LIMIT];
static mut PASSES: [[[u128; 4]; COUNT]; LIMIT] = [[[0; 4]; COUNT]; LIMIT];
static mut BASE: [u128; LIMIT] = [0; LIMIT];
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = core::ptr::null_mut();
#[unsafe(no_mangle)]
static mut native_prop_continuation_deadline: u64 = 0;
unsafe extern "C" {
    static mut native_weighted_arm: [u64; 10];
    static native_cyclic_idle_landing: u8;
}
struct Runtime {
    p: Proportional<COUNT>,
    hz: u64,
    owner: usize,
    sleepers: [Option<u64>; COUNT],
    exited: u32,
    cumulative: [u64; COUNT],
    start: u64,
    compare: u64,
    expiry: u64,
    fresh: bool,
    grants: u64,
    invalid: Option<u64>,
    used: usize,
    lost: u64,
    row: Option<usize>,
}
static mut STATE: Option<Runtime> = None;
fn state() -> &'static mut Runtime {
    unsafe {
        (&mut *(&raw mut STATE))
            .as_mut()
            .unwrap_or_else(|| guard(30, 0, 1))
    }
}
fn idle_top() -> *mut u8 {
    (&raw mut IDLE)
        .cast::<u8>()
        .wrapping_add(context::STACK_BYTES)
}
fn registers() -> (u64, u64, u64) {
    let (cval, ctl, root);
    unsafe {
        asm!("mrs {}, cntp_cval_el0","mrs {}, cntp_ctl_el0","mrs {}, ttbr0_el1",
        out(reg)cval,out(reg)ctl,out(reg)root,options(nostack,preserves_flags));
    }
    (cval, ctl, root)
}
// Single CPU, masked kernel path. Remains readable if normal terminal dump is unreachable.
static mut FIRST_REJECTION: FirstRejection = FirstRejection(None);
static mut GUARD_EVENT: [u64; 6] = [0; 6];

fn guard(code: u64, actual: u64, expected: u64) -> ! {
    rejected(code, Rejection(0, actual as u128, expected as u128, 0))
}

fn rejected(code: u64, reason: Rejection) -> ! {
    let words = unsafe { *(&mut *(&raw mut FIRST_REJECTION)).record(code, reason) };
    let actual = words[2];
    let expected = words[4];

    let (cval, ctl, root) = registers();
    let (pc, pstate, esr, sp, far): (u64, u64, u64, u64, u64);
    unsafe {
        asm!("mrs {}, elr_el1","mrs {}, spsr_el1","mrs {}, esr_el1","mrs {}, sp_el0","mrs {}, far_el1",
        out(reg)pc,out(reg)pstate,out(reg)esr,out(reg)sp,out(reg)far,options(nostack,preserves_flags));
    }
    let stamp = counter();
    print(b"PREDICATE", &words);
    print(b"GUARD_EVENT", &unsafe { GUARD_EVENT });
    print(
        b"FAULT",
        &[
            code, actual, expected, stamp, cval, ctl, root, pc, pstate, esr, sp, far,
        ],
    );
    fail(24)
}
pub(super) fn validate_sync(
    vector: u64,
    f: &NativeFrame,
    vbar: u64,
    ttbr: u64,
    far: u64,
    entry: u64,
) {
    unsafe {
        GUARD_EVENT = [2, f.x[8], entry, entry, f.x[0], f.x[1]];
    }
    let expected = &raw const native_el0_vectors as u64;
    let checks = [
        (vector, 0x400),
        (vbar, expected),
        (f.current_el, 4),
        (f.spsr & 31, 0),
        (f.esr >> 26, 0x15),
        (f.esr & 0xffff, 0x4a01),
        (
            native_el0_mmu::user_range(f.elr as usize, 4, false) as u64,
            1,
        ),
        (
            ((STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&f.sp_el0)) as u64,
            1,
        ),
        (f.sp_el0 % 16, 0),
    ];
    for (i, (actual, want)) in checks.into_iter().enumerate() {
        if actual != want {
            print(
                b"CONTEXT",
                &[entry, ttbr, far, f.elr, f.sp_el0, f.spsr, f.esr],
            );
            guard(100 + i as u64, actual, want);
        }
    }
    if f.x[8] == 1 && f.x[0] != f.x[0] as i32 as i64 as u64 {
        guard(109, f.x[0], f.x[0] as i32 as i64 as u64);
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_weighted_arm_overflow() -> ! {
    let arm = unsafe { ptr::read_volatile(&raw const native_weighted_arm) };
    guard(20, arm[3], u64::MAX - arm[1])
}

pub(super) fn start(hz: u64) -> usize {
    if !(1..=3).contains(&COUNT) {
        rejected(26, Rejection(12, COUNT as u128, 1, 3));
    }
    if NATIVE_QUANTUM_TICKS == 0 {
        guard(27, 0, 1);
    }
    if NATIVE_QUANTUM_TICKS > i64::MAX as u64 {
        guard(1, NATIVE_QUANTUM_TICKS, i64::MAX as u64);
    }
    if !matches!(NATIVE_POLICY, "lottery" | "stride") {
        uart_write(b"NATIVE PROP MODE ");
        uart_write(NATIVE_POLICY.as_bytes());
        uart_write(b"\n");
        guard(28, 0, 1);
    }
    if !LOTTERY && NATIVE_LOTTERY_SEED != 0 {
        guard(25, NATIVE_LOTTERY_SEED, 0);
    }
    let weights = NATIVE_WEIGHTS.map(|w| u8::try_from(w).unwrap_or(0));
    let mut p = if LOTTERY {
        Proportional::lottery(weights, NATIVE_LOTTERY_SEED)
    } else {
        Proportional::stride(weights)
    }
    .unwrap_or_else(|| {
        let id = NATIVE_WEIGHTS
            .iter()
            .position(|w| !(1..=16).contains(w))
            .unwrap();
        rejected(2, Rejection(17, NATIVE_WEIGHTS[id] as u128, 16, id as u128))
    });
    if NATIVE_LOTTERY_HORIZON != 0
        || NATIVE_LOTTERY_WINDOW != 0
        || NATIVE_LOTTERY_FLOORS.iter().any(|&f| f != 0)
    {
        if !LOTTERY {
            guard(3, 0, 1);
        }
        let report = NativeWindowInput {
            seed: NATIVE_LOTTERY_SEED,
            weights,
            horizon: NATIVE_LOTTERY_HORIZON,
            width: NATIVE_LOTTERY_WINDOW,
            floors: NATIVE_LOTTERY_FLOORS,
        }
        .try_analyze()
        .unwrap_or_else(|reason| rejected(4, reason));
        if let Some((s, id, count)) = report.first_short {
            print(
                b"REFUSED",
                &[
                    s as u64,
                    id as u64,
                    count as u64,
                    NATIVE_LOTTERY_FLOORS[id] as u64,
                ],
            );
            fail(24)
        }
    }
    p.try_mask(ALL).unwrap_or_else(|reason| rejected(5, reason));
    unsafe {
        STATE = Some(Runtime {
            p,
            hz,
            owner: IDLE_OWNER,
            sleepers: [None; COUNT],
            exited: 0,
            cumulative: [0; COUNT],
            start: 0,
            expiry: 0,
            compare: 0,
            fresh: false,
            grants: 0,
            invalid: None,
            used: 0,
            lost: 0,
            row: None,
        });
    }
    let now = counter();
    step(now, now, 0, 0, 0, 0, None, Action::Keep)
}

fn import_arm(r: &mut Runtime) {
    if !r.fresh {
        return;
    }
    let a = unsafe { ptr::read_volatile(&raw const native_weighted_arm) };
    if a[0] != 0 {
        guard(6, a[0], 0);
    }
    r.start = a[3];
    r.expiry = r
        .start
        .checked_add(NATIVE_QUANTUM_TICKS)
        .unwrap_or_else(|| guard(7, r.start, u64::MAX - NATIVE_QUANTUM_TICKS));
    r.compare = r
        .sleepers
        .iter()
        .flatten()
        .copied()
        .fold(r.expiry, u64::min);
    if a[4] != r.compare {
        guard(8, a[4], r.compare);
    }
    if a[5] & 3 != 1 {
        guard(9, a[5], 1);
    }
    if let Some(i) = r.row {
        unsafe {
            EVENTS[i][31] = a[3];
            EVENTS[i][32] = a[4];
            EVENTS[i][33] = a[5];
            EVENTS[i][35] = a[7];
            EVENTS[i][29] = a[8];
        }
    }
    r.fresh = false;
}
#[unsafe(no_mangle)]
extern "C" fn native_prop_return() {
    let r = state();
    if r.fresh {
        unsafe {
            ptr::write_volatile(&raw mut native_weighted_arm[1], NATIVE_QUANTUM_TICKS);
            ptr::write_volatile(
                &raw mut native_weighted_arm[2],
                r.sleepers
                    .iter()
                    .flatten()
                    .copied()
                    .min()
                    .unwrap_or(u64::MAX),
            );
            ptr::write_volatile(&raw mut native_weighted_arm[0], 1);
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    if let Some(i) = state().row {
        unsafe {
            EVENTS[i][29] = registers().2;
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    if let Some(i) = state().row {
        unsafe {
            EVENTS[i][36] = stamp;
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    Keep,
    Yield,
    Sleep(u64),
    Exit,
}
fn mask(r: &mut Runtime, bits: u32) {
    r.p.try_mask(bits)
        .unwrap_or_else(|reason| rejected(10, reason));
}
fn passes(r: &Runtime) -> [u128; COUNT] {
    core::array::from_fn(|id| r.p.pass_wide(id).unwrap_or(0))
}

fn step(
    entry: u64,
    decision: u64,
    kind: u64,
    op: u64,
    arg: u64,
    result: u64,
    f: Option<&NativeFrame>,
    action: Action,
) -> usize {
    unsafe {
        GUARD_EVENT = [kind, op, entry, decision, arg, result];
    }
    let r = state();
    import_arm(r);
    let (cval, ctl, root) = registers();
    let src = r.owner;
    let old = r.compare;
    let old_r = r.start;
    let before = r.p.ready_mask();
    let due = old != 0 && decision >= old;
    if kind == 1 {
        if op & 0x3ff != u64::from(context::TIMER_IRQ) {
            guard(11, op & 0x3ff, u64::from(context::TIMER_IRQ));
        }
        if cval != old {
            guard(12, cval, old);
        }
        if entry < old {
            guard(13, entry, old);
        }
        if ctl & 3 != 1 {
            guard(14, ctl, 1);
        }
    }
    let close = kind == 0 || due || !matches!(action, Action::Keep);
    let mut stop = 0;
    let mut dt = 0;
    let full = src < COUNT && kind == 1 && old == r.expiry && entry >= r.expiry;
    if close && src < COUNT {
        stop = if matches!(action, Action::Keep) && entry < old {
            decision
        } else {
            entry
        };
        dt = stop
            .checked_sub(old_r)
            .unwrap_or_else(|| guard(15, stop, old_r));
        let total = r.cumulative[src]
            .checked_add(dt)
            .unwrap_or_else(|| guard(16, dt, u64::MAX - r.cumulative[src]));
        r.p.try_stop_elapsed(src, dt)
            .unwrap_or_else(|reason| rejected(17, reason));
        r.cumulative[src] = total;
        if r.grants <= NATIVE_LOTTERY_HORIZON as u64
            && (!full || before != ALL)
            && r.invalid.is_none()
        {
            r.invalid = Some(r.grants - 1);
        }
    }
    let charged = passes(r);
    let sleep = if let Action::Sleep(at) = action {
        at
    } else {
        0
    };
    if matches!(action, Action::Exit | Action::Sleep(_)) {
        let reduced = r.p.ready_mask() & !(1 << src);
        mask(r, reduced);
        if sleep != 0 {
            r.sleepers[src] = Some(sleep);
        } else {
            r.exited |= 1 << src;
        }
    }
    let mut woke = 0u32;
    while let Some((_, id)) = r
        .sleepers
        .iter()
        .enumerate()
        .filter_map(|(id, &at)| at.map(|v| (v, id)))
        .filter(|&(v, _)| v <= decision)
        .min()
    {
        r.sleepers[id] = None;
        woke |= 1 << id;
        let added = r.p.ready_mask() | 1 << id;
        mask(r, added);
    }
    let pre = passes(r);
    let next = if close {
        if r.exited == ALL {
            TERMINAL
        } else if r.p.ready_mask() == 0 {
            IDLE_OWNER
        } else {
            r.p.try_dispatch()
                .unwrap_or_else(|reason| rejected(18, reason))
        }
    } else {
        src
    };
    let selected = close && next < COUNT;
    let ordinal = if selected {
        let n = r.grants;
        r.grants = n
            .checked_add(1)
            .unwrap_or_else(|| guard(19, n, u64::MAX - 1));
        n
    } else {
        NO_PID
    };
    if close {
        r.start = 0;
        r.expiry = 0;
        r.compare = 0;
        r.fresh = selected;
        if !selected {
            let wake = r.sleepers.iter().flatten().copied().min();
            if next == IDLE_OWNER && wake.is_none() {
                guard(21, 0, 1);
            }
            arm_compare(wake);
            r.compare = wake.unwrap_or(0);
        }
    }
    unsafe {
        native_prop_continuation_deadline = if !close && next < COUNT { old } else { 0 };
    }
    let dst = if next >= COUNT {
        None
    } else if next == src && f.is_some() {
        f
    } else {
        Some(unsafe { &*(&raw const FRAMES[next]) })
    };
    let (src_pc, src_sp, src_ps) = if let Some(f) = f {
        (f.elr, f.sp_el0, f.spsr)
    } else if kind == 1 {
        let (pc, ps);
        unsafe {
            asm!("mrs {},elr_el1","mrs {},spsr_el1",out(reg)pc,out(reg)ps,options(nostack,preserves_flags));
        }
        (pc, idle_top() as u64, ps)
    } else {
        (0, 0, 0)
    };
    let (dst_pc, dst_sp, dst_ps) = dst.map_or_else(
        || {
            if next == IDLE_OWNER {
                (
                    &raw const native_cyclic_idle_landing as u64,
                    idle_top() as u64,
                    5,
                )
            } else {
                (0, 0, 0)
            }
        },
        |f| (f.elr, f.sp_el0, f.spsr),
    );
    let i = r.used;
    if i < LIMIT {
        unsafe {
            EVENTS[i] = [
                i as u64,
                kind,
                entry,
                decision,
                op,
                arg,
                result,
                src as u64,
                next as u64,
                selected as u64,
                if close && src < COUNT { old_r } else { 0 },
                stop,
                dt,
                old,
                cval,
                ctl,
                before as u64,
                r.p.ready_mask() as u64,
                woke as u64,
                r.exited as u64,
                sleep,
                src_pc,
                src_sp,
                src_ps,
                root,
                if src < COUNT {
                    (&raw const FRAMES[src]).addr() as u64
                } else {
                    0
                },
                dst_pc,
                dst_sp,
                dst_ps,
                if next < COUNT {
                    native_el0_mmu::space_id(next).unwrap_or(0)
                } else {
                    root
                },
                if next < COUNT {
                    (&raw const FRAMES[next]).addr() as u64
                } else {
                    0
                },
                0,
                if selected { 0 } else { r.compare },
                if selected { 0 } else { registers().1 },
                0,
                0,
                0,
                0,
                full as u64,
                ordinal,
            ];
            if selected && LOTTERY {
                let d = r.p.draw_record().unwrap_or_else(|| guard(22, 0, 1));
                DRAWS[i] = [
                    d.before,
                    d.after,
                    d.words,
                    d.raw,
                    d.ticket as u64,
                    d.total as u64,
                ];
            }
            BASE[i] = if selected {
                r.p.last_normalization_wide().unwrap_or(0)
            } else {
                0
            };
            for id in 0..COUNT {
                PASSES[i][id] = [
                    charged[id],
                    pre[id],
                    r.p.pass_wide(id).unwrap_or(0),
                    u128::from(r.cumulative[id]),
                ];
            }
            native_cyclic_return_cell = &raw mut EVENTS[i][34];
        }
        r.row = Some(i);
        r.used += 1;
    } else {
        r.row = None;
        r.lost = r.lost.saturating_add(1);
        unsafe {
            native_cyclic_return_cell = core::ptr::null_mut();
        }
    }
    r.owner = next;
    next
}

fn transfer(next: usize, f: &mut NativeFrame) {
    if next == TERMINAL {
        finish();
    }
    if next == IDLE_OWNER {
        unsafe {
            FRAMES[CURRENT] = *f;
            native_sleep_switch(idle_top());
        }
        activate(state().owner, f);
    } else {
        activate(next, f);
    }
}
#[unsafe(no_mangle)]
extern "C" fn native_prop_late(entry: u64, f: &mut NativeFrame) {
    let next = step(entry, counter(), 3, 0, 0, 0, Some(f), Action::Keep);
    transfer(next, f);
}
pub(super) fn irq(entry: u64, ptr: *mut NativeFrame, iar: u32) -> u64 {
    unsafe {
        GUARD_EVENT = [1, iar as u64, entry, entry, ptr as u64, 0];
    }
    let f = unsafe { ptr.as_mut() };
    if f.is_some() != (state().owner < COUNT) {
        guard(23, f.is_some() as u64, (state().owner < COUNT) as u64);
    }
    let next = step(
        entry,
        entry,
        1,
        u64::from(iar),
        0,
        0,
        f.as_deref(),
        Action::Keep,
    );
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    context::gic_write(context::GICC + 0x10, iar);
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    if let Some(i) = state().row {
        unsafe {
            EVENTS[i][37] = iar as u64;
        }
    }
    if let Some(f) = f {
        transfer(next, f);
    }
    (next < COUNT) as u64
}
pub(super) fn syscall(entry: u64, f: &mut NativeFrame) {
    unsafe {
        GUARD_EVENT = [2, f.x[8], entry, entry, f.x[0], f.x[1]];
    }
    let src = state().owner;
    if src >= COUNT {
        guard(29, src as u64, COUNT as u64);
    }
    if src != unsafe { CURRENT } {
        guard(24, src as u64, unsafe { CURRENT } as u64);
    }
    let op = f.x[8];
    let arg = f.x[0];
    let mut action = Action::Keep;
    let result = match op {
        1 => {
            unsafe {
                EXIT_STATUS[src] = arg as i32;
            }
            head(9, arg);
            action = Action::Exit;
            arg as i64
        }
        6 => clock_gettime(arg, f.x[1] as usize, entry),
        7 => match sleep_until(f) {
            Err(e) => e,
            Ok(at) if at.saturating_sub(entry) > i64::MAX as u64 => -75,
            Ok(at) => {
                if at > entry {
                    action = Action::Sleep(at);
                }
                0
            }
        },
        8 => {
            action = Action::Yield;
            0
        }
        9..=18 => -95,
        _ => -38,
    };
    f.x[0] = result as u64;
    let next = step(entry, counter(), 2, op, arg, result as u64, Some(f), action);
    transfer(next, f);
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(b"NATIVE PROP ");
    uart_write(label);
    for &v in fields {
        uart_write(b" ");
        unsigned(v);
    }
    uart_write(b"\n");
}
fn wide(mut v: u128) {
    let mut buf = [0u8; 39];
    let mut n = buf.len();
    loop {
        n -= 1;
        buf[n] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    uart_write(&buf[n..]);
}
fn finish() -> ! {
    let r = state();
    print(
        b"CONFIG",
        &[
            1,
            (!LOTTERY) as u64,
            r.hz,
            NATIVE_QUANTUM_TICKS,
            NATIVE_LOTTERY_SEED,
            NATIVE_LOTTERY_HORIZON as u64,
            NATIVE_LOTTERY_WINDOW as u64,
            COUNT as u64,
        ],
    );
    for id in 0..COUNT {
        print(
            b"SLOT",
            &[
                id as u64,
                NATIVE_WEIGHTS[id] as u64,
                NATIVE_LOTTERY_FLOORS[id] as u64,
                native_el0_mmu::space_id(id).unwrap_or(0),
            ],
        );
    }
    unsafe {
        for i in 0..r.used {
            print(b"EVENT", &*(&raw const EVENTS[i]));
            if LOTTERY && EVENTS[i][9] == 1 {
                uart_write(b"NATIVE PROP DRAW ");
                unsigned(i as u64);
                for v in DRAWS[i] {
                    uart_write(b" ");
                    unsigned(v);
                }
                uart_write(b"\n");
            }
            if !LOTTERY {
                uart_write(b"NATIVE PROP BASE ");
                unsigned(i as u64);
                uart_write(b" ");
                wide(BASE[i]);
                uart_write(b"\n");
            }
            for id in 0..COUNT {
                uart_write(b"NATIVE PROP PASS ");
                unsigned(i as u64);
                uart_write(b" ");
                unsigned(id as u64);
                for v in PASSES[i][id] {
                    uart_write(b" ");
                    wide(v);
                }
                uart_write(b"\n");
            }
        }
    }
    print(b"CONDITION", &[r.invalid.unwrap_or(NO_PID)]);
    print(b"DROPPED", &[r.lost]);
    for id in 0..COUNT {
        let status = i64::from(unsafe { EXIT_STATUS[id] });
        if COUNT == 1 {
            uart_write(b"NATIVE EL0 EXIT ");
            decimal(status);
            uart_write(b"\n");
        } else {
            event(b"EXIT", id, Some(status));
        }
    }
    head(10, 1);
    head(1, 3);
    park()
}
