#![no_std]
#![no_main]

#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{state, status, Bench, Case, Timeline, ACK, READY, STOP};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{
    exit, ipc, loan,
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
#[no_mangle]
static mut safety_active_ipc: [[u64; 4]; 4] = [[0; 4]; 4];
#[no_mangle]
static mut safety_wrong_owner: [u64; 14] = [0; 14];
#[no_mangle]
static mut safety_rearm_launch: [u64; 4] = [0; 4];
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
        self.capture_matching(planned, cmd, gen, Some(want_rc), |_, after| {
            enable.is_none_or(|v| v == (after.flags & dev::ENABLED != 0))
                && seq.is_none_or(|s| s == after.last_sequence)
        })
    }
    fn capture_matching(
        &mut self,
        planned: u64,
        cmd: Option<Command>,
        gen: u64,
        want_rc: Option<i64>,
        matches: impl FnOnce(&dev::State, &dev::State) -> bool,
    ) -> Bench<()> {
        let before = state()?;
        let rc = cmd.map_or(0, |c| status(dev::supervise(c, gen)));
        let after = state()?;
        // Calibration accepts either ordinary REARM outcome. Exact decisions are
        // scored against the kernel's recorded operand, not the planned release.
        let ok =
            want_rc.map_or(matches!(rc, 0 | -11), |want| rc == want) && matches(&before, &after);
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
                    want_rc.unwrap_or(i64::MIN) as u64,
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
    fn observe_active(
        &mut self,
        ms: u64,
        gen: u64,
        seq: u64,
        issue_ms: u64,
        enabled: bool,
    ) -> Bench<()> {
        self.time.wait(ms)?;
        let issue = self.time.at(issue_ms)?;
        let hz = self.time.hz;
        self.capture_matching(self.time.at(ms)?, None, gen, Some(0), |before, after| {
            [before, after]
                .into_iter()
                .all(|s| active_state(s, hz, gen, seq, issue, enabled))
        })
    }
    fn prime_active(&mut self, offset: u64, generation: u64) -> Bench<()> {
        self.observe_active(offset + 3, generation, 1, offset, false)?;
        self.observe_active(offset + 13, generation, 2, offset + 10, false)?;
        self.command(offset + 15, Command::Rearm, generation, 0)?;
        self.observe_active(offset + 18, generation, 2, offset + 10, true)
    }
    fn rearm_at(&mut self, ms: u64, lead_ticks: u64) -> Bench<()> {
        let prior_failures = self.failed;
        self.observe_active(13, 1, 2, 10, false)?;
        if self.failed != prior_failures {
            return Err(95);
        }
        let target = self.time.at(ms)?;
        let wake = target.checked_sub(lead_ticks).ok_or(83)?;
        let observed_wake = self.time.until(wake)?;
        self.capture_matching(target, Some(Command::Rearm), 1, None, |_, _| true)?;
        unsafe {
            write_volatile(
                &raw mut safety_rearm_launch,
                [target, lead_ticks, wake, observed_wake],
            );
        }
        self.time.wait(ms + 3)?;
        self.capture(self.time.at(ms + 3)?, None, 1, 0, None, Some(2))?;
        self.observe(35, false, Some(2))
    }
    fn active_reply(&mut self, row: usize, pid: u64, word: u64, expected: u64) -> bool {
        let reply = ipc::call(pid, word, u64::MAX);
        let ok = reply == Ok(expected);
        if !ok {
            self.failed += 1;
        }
        unsafe {
            write_volatile(
                &raw mut safety_active_ipc[row],
                [pid, word, reply.unwrap_or(0), status(reply) as u64],
            );
        }
        ok
    }
    fn wrong_owner(&mut self, generation: u64, issue_ms: u64) -> Bench<()> {
        let before = state()?;
        let rc = status(loan::acquire());
        let observed = state();
        let close_status = if rc == 0 {
            // Revoke the unexpected grant and the staged loan without accessing either page.
            status(dev::supervise(Command::Close, generation)) as u64
        } else {
            1 << 63
        };
        let after = observed?;
        let issue = self.time.at(issue_ms)?;
        let ok = rc == -13
            && [&before, &after]
                .into_iter()
                .all(|s| active_state(s, self.time.hz, generation, 3, issue, true));
        unsafe {
            write_volatile(
                &raw mut safety_wrong_owner,
                [
                    1,
                    before.ticks,
                    after.ticks,
                    rc as u64,
                    before.generation,
                    after.generation,
                    before.flags,
                    after.flags,
                    before.last_sequence,
                    after.last_sequence,
                    before.last_issue_ticks,
                    after.last_issue_ticks,
                    ok as u64,
                    close_status,
                ],
            );
        }
        if !ok {
            self.failed += 1;
        }
        Ok(())
    }
    fn active_scenario(&mut self, case: Case) -> Bench<()> {
        let offset = case.active_offset_ms();
        let generation = case.active_generation();
        let prior = if case == Case::OldGeneration {
            self.prime_active(0, 1)
                .and_then(|()| self.observe_active(23, 1, 3, 20, true))
                .and_then(|()| self.command(35, Command::Open, generation, 0))
        } else {
            Ok(())
        };
        let primed = self.prime_active(offset, generation);
        self.active_reply(0, 1, common::LAST_SAMPLE, 3);
        let live = self.observe_active(offset + 21, generation, 3, offset + 20, true);
        let released = self.active_reply(1, 0, common::RELEASE_INPUT, common::RELEASE_INPUT);
        let seq4_issue = if case == Case::WrongOwner {
            if released && self.active_reply(2, 0, common::GRANT_STAGED, common::GRANT_STAGED) {
                let denial = self.wrong_owner(generation, offset + 20);
                // The owner waits for this reply only after acknowledging its staged grant.
                self.active_reply(3, 0, common::PUBLISH_GRANT, common::PUBLISH_GRANT);
                denial?;
            }
            offset + 24
        } else {
            offset + 30
        };
        prior?;
        primed?;
        live?;
        self.observe_active(offset + 28, generation, 3, offset + 20, true)?;
        self.observe_active(offset + 33, generation, 4, seq4_issue, true)?;
        self.observe_active(offset + 42, generation, 5, offset + 36, false)
    }
    fn observe_recovery(
        &mut self,
        ms: u64,
        generation: u64,
        seq: u64,
        issue_ms: u64,
        flags: u64,
    ) -> Bench<()> {
        let target = self.time.at(ms)?;
        let issue = self.time.at(issue_ms)?;
        self.time.until(target)?;
        self.capture_matching(target, None, generation, Some(0), |before, after| {
            before.ticks >= target
                && after.ticks >= before.ticks
                && [before, after]
                    .into_iter()
                    .all(|s| recovery_state(s, generation, seq, issue, flags))
        })
    }
    fn recover(&mut self, case: Case) -> Bench<()> {
        let open = dev::GATE_OPEN;
        let high = open | dev::ENABLED;
        let latched = open | dev::FAULT;
        self.prime_active(0, 1)?;
        self.observe_recovery(23, 1, 3, 20, high)?;
        self.observe_recovery(48, 1, 3, 20, latched)?;
        if self.failed != 0 {
            return Err(95);
        }
        let old_issue = self.time.at(20)?;
        self.time.wait(50)?;
        self.capture_matching(
            self.time.at(50)?,
            Some(Command::Open),
            2,
            Some(0),
            |before, after| {
                recovery_state(before, 1, 3, old_issue, latched)
                    && recovery_state(after, 2, 0, 0, open)
            },
        )?;
        let first_issue = self.time.at(55)?;
        self.time.wait(60)?;
        self.capture_matching(
            self.time.at(60)?,
            Some(Command::Rearm),
            2,
            Some(-11),
            |before, after| {
                [before, after]
                    .into_iter()
                    .all(|s| recovery_state(s, 2, 1, first_issue, open))
            },
        )?;
        self.observe_recovery(68, 2, 2, 65, open)?;
        if case == Case::RecoverLatch {
            self.command(70, Command::Rearm, 2, 0)?;
            self.observe_recovery(73, 2, 2, 65, high)?;
            self.observe_recovery(82, 2, 3, 76, open)
        } else {
            self.observe_recovery(73, 2, 2, 65, open)?;
            self.observe_recovery(82, 2, 2, 65, open)
        }
    }
    fn scenario(&mut self, case: Case) -> Bench<()> {
        use Case::*;
        if matches!(case, RecoverLatch | RecoverHold) {
            return self.recover(case);
        }
        if case.active_case() {
            return self.active_scenario(case);
        }
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
            Silence | Phase | PhaseDecision | PhaseComplete => {
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
fn recovery_state(s: &dev::State, generation: u64, seq: u64, issue: u64, flags: u64) -> bool {
    s.generation == generation
        && s.last_sequence == seq
        && s.last_issue_ticks == issue
        && s.flags & (dev::GATE_OPEN | dev::ENABLED | dev::FAULT | dev::PENDING_REARM) == flags
}
fn active_state(
    s: &dev::State,
    hz: u64,
    generation: u64,
    seq: u64,
    issue: u64,
    enabled: bool,
) -> bool {
    let flags = dev::GATE_OPEN | if enabled { dev::ENABLED } else { 0 };
    s.generation == generation
        && s.last_sequence == seq
        && s.last_issue_ticks == issue
        && s.flags & (dev::GATE_OPEN | dev::ENABLED | dev::FAULT) == flags
        && s.hz == hz
        && s.ticks
            .checked_sub(issue)
            .is_some_and(|age| u128::from(age) * 1000 < u128::from(hz) * 20)
}
fn call(pid: u64, word: u64, want: u64) -> Bench<()> {
    if ipc::call(pid, word, u64::MAX) == Ok(want) {
        Ok(())
    } else {
        Err(84)
    }
}
fn run(case: Case, lead_ticks: u64) -> Bench<()> {
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
    let result = match case.rearm_target_ms() {
        Some(ms) => s.rearm_at(ms, lead_ticks),
        None => s.scenario(case),
    };
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
    (|| {
        if !matches!(argc, 2 | 3) {
            return Err(81);
        }
        let case = Case::parse(2, argv)?;
        let mut lead_ticks = 0;
        if argc == 3 {
            if case.rearm_target_ms().is_none() {
                return Err(81);
            }
            lead_ticks = common::parse_lead_ticks(*argv.add(2))?;
        }
        run(case, lead_ticks)
    })()
    .err()
    .unwrap_or(73)
}
