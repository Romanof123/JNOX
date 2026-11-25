#![no_std]
#![no_main]

#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{state, status, Bench, Case, Timeline, ACK, READY, STOP};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{
    exit, ipc,
    safety_channel::{self as dev, Command},
};

#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut safety_supervisor_header: [u64; 6] = [0x53465032, 0, 0, 0, 0, 0];
#[no_mangle]
static mut safety_supervisor_rows: [[u64; 16]; 48] = [[0; 16]; 48];
#[no_mangle]
static mut safety_supervisor_count: usize = 0;
#[no_mangle]
static mut safety_live_startup: [u64; 5] = [0; 5];
struct Supervisor {
    time: Timeline,
    failed: u64,
}
impl Supervisor {
    fn capture(
        &mut self,
        planned: u64,
        cmd: Option<Command>,
        gen: u64,
        want_rc: i64,
        enable: Option<bool>,
        seq: Option<u64>,
    ) -> Bench<()> {
        let before = state()?;
        let rc = cmd.map_or(0, |c| status(dev::supervise(c, gen)));
        let after = state()?;
        let ok = rc == want_rc
            && enable.is_none_or(|v| v == (after.flags & dev::ENABLED != 0))
            && seq.is_none_or(|s| s == after.last_sequence);
        if !ok {
            self.failed += 1;
        }
        unsafe {
            let i = safety_supervisor_count;
            if i >= 48 {
                return Err(87);
            }
            write_volatile(
                &raw mut safety_supervisor_rows[i],
                [
                    planned,
                    before.ticks,
                    after.ticks,
                    cmd.map_or(0, |c| c as u64),
                    gen,
                    rc as u64,
                    want_rc as u64,
                    before.flags,
                    after.flags,
                    before.generation,
                    after.generation,
                    before.last_sequence,
                    after.last_sequence,
                    before.last_issue_ticks,
                    after.last_issue_ticks,
                    ok as u64,
                ],
            );
            write_volatile(&raw mut safety_supervisor_count, i + 1);
        }
        Ok(())
    }
    fn command(&mut self, ms: u64, cmd: Command, gen: u64, want: i64) -> Bench<()> {
        self.time.wait(ms)?;
        self.capture(self.time.at(ms)?, Some(cmd), gen, want, None, None)
    }
    fn observe(&mut self, ms: u64, enabled: bool, seq: Option<u64>) -> Bench<()> {
        self.time.wait(ms)?;
        self.capture(self.time.at(ms)?, None, 0, 0, Some(enabled), seq)
    }
    fn scenario(&mut self, case: Case) -> Bench<()> {
        use Case::*;
        self.observe(3, false, if case == Delayed { Some(0) } else { Some(1) })?;
        if case == LiveStartup {
            let a = state()?;
            self.observe(13, false, Some(2))?;
            let b = state()?;
            let eligible = a.last_sequence == 1
                && b.last_sequence == 2
                && b.last_issue_ticks.checked_sub(a.last_issue_ticks)
                    == Some(self.time.at(10)? - self.time.epoch);
            self.time.wait(15)?;
            let call_tick = state()?.ticks;
            let fresh = call_tick
                .checked_sub(a.last_issue_ticks)
                .is_some_and(|age| u128::from(age) * 1000 < u128::from(self.time.hz) * 20);
            let expected = eligible && fresh;
            unsafe {
                write_volatile(
                    &raw mut safety_live_startup,
                    [
                        a.last_issue_ticks,
                        b.last_issue_ticks,
                        call_tick,
                        eligible as u64,
                        expected as u64,
                    ],
                );
            }
            self.capture(
                self.time.at(15)?,
                Some(Command::Rearm),
                1,
                if expected { 0 } else { -11 },
                None,
                None,
            )?;
            self.observe(18, expected, Some(2))?;
            for (ms, seq) in [(23, 3), (33, 4), (43, 5), (53, 6), (58, 6)] {
                self.observe(ms, expected, Some(seq))?;
            }
            return self.observe(63, false, Some(7));
        }
        if case == Delayed {
            self.observe(28, false, Some(0))?;
            self.command(45, Command::Rearm, 1, 0)?;
            self.observe(48, true, Some(2))?;
            self.observe(63, false, Some(4))?;
            return Ok(());
        }
        if case == Early {
            self.command(6, Command::Rearm, 1, -11)?;
            self.observe(13, false, Some(2))?;
        }
        if matches!(case, One | BadSpacing | Boundary | NoRearm) {
            if case != NoRearm {
                self.command(
                    if case == Boundary { 20 } else { 15 },
                    Command::Rearm,
                    1,
                    -11,
                )?;
            }
            self.observe(24, false, None)?;
            self.observe(45, false, None)?;
            return Ok(());
        }
        self.command(15, Command::Rearm, 1, 0)?;
        self.observe(18, true, Some(2))?;
        if case == Early {
            return self.observe(35, false, Some(2));
        }
        match case {
            Healthy | Observed => {
                for (ms, seq) in [(23, 3), (33, 4), (43, 5), (53, 6), (58, 6)] {
                    self.observe(ms, true, Some(seq))?;
                }
                self.observe(63, false, Some(7))?;
            }
            Silence | Phase => {
                // The controller answers immediately after the activation that
                // consumes the final good sample. Close on return from CALL.
                call(1, common::LAST_SAMPLE, 3)?;
                self.capture(
                    self.time.at(20)?,
                    Some(Command::Close),
                    1,
                    0,
                    Some(true),
                    Some(3),
                )?;
                self.observe(32, true, Some(3))?;
                self.observe(48, false, Some(3))?;
                self.observe(65, false, Some(3))?;
            }
            Queued | Invalid | Saturation => {
                self.observe(22, true, Some(3))?;
                self.command(
                    if case == Queued { 24 } else { 23 },
                    if matches!(case, Invalid | Saturation) {
                        Command::Mark
                    } else {
                        Command::Close
                    },
                    1,
                    0,
                )?;
                self.observe(32, true, Some(3))?;
                self.observe(48, false, Some(3))?;
                self.observe(65, false, Some(3))?;
            }
            Collision => {
                self.command(30, Command::Mark, 1, 0)?;
                self.observe(37, true, Some(3))?;
                self.observe(47, false, Some(3))?;
            }
            Restart => {
                self.observe(33, false, Some(4))?;
                self.command(35, Command::Close, 1, 0)?;
                self.command(40, Command::Open, 2, 0)?;
                self.command(48, Command::Rearm, 1, -116)?;
                self.command(49, Command::Rearm, 2, -11)?;
                self.observe(53, false, Some(1))?;
                self.command(59, Command::Rearm, 2, 0)?;
                self.observe(63, true, Some(2))?;
                self.observe(78, false, Some(4))?;
            }
            _ => {}
        }
        Ok(())
    }
}
fn call(pid: u64, word: u64, want: u64) -> Bench<()> {
    if ipc::call(pid, word, u64::MAX) == Ok(want) {
        Ok(())
    } else {
        Err(84)
    }
}
fn run(case: Case) -> Bench<()> {
    let first = state()?;
    let schedule = Timeline::new(first.ticks, first.hz)?;
    let time = Timeline::new(schedule.at(50)?, first.hz)?;
    let mut s = Supervisor { time, failed: 0 };
    s.capture(first.ticks, Some(Command::Open), 1, 0, Some(false), Some(0))?;
    s.capture(
        first.ticks,
        Some(Command::Open),
        1,
        -116,
        Some(false),
        Some(0),
    )?;
    for pid in [0, 1] {
        call(pid, time.epoch, ACK | case as u64)?;
    }
    let result = s.scenario(case);
    time.wait(85)?;
    // Both applications acknowledge READY after all private memory accesses,
    // before any permitted process exit can revoke an outstanding loan.
    for word in [READY, STOP] {
        for pid in [0, 1] {
            call(pid, word, word)?;
        }
    }
    unsafe {
        write_volatile(
            &raw mut safety_supervisor_header,
            [
                0x53465032,
                case as u64,
                time.epoch,
                time.hz,
                s.failed,
                result.as_ref().err().copied().unwrap_or(0) as u64,
            ],
        );
    }
    result?;
    if s.failed == 0 {
        Ok(())
    } else {
        Err(95)
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}
#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    Case::parse(argc, argv).and_then(run).err().unwrap_or(73)
}
