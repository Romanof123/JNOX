#![no_std]
#![no_main]

#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{state, status, Bench, Case, Timeline};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, ipc, loan, safety_channel as dev};
use jnox_safety_channel::{encode, validate_loan, Error, Permit, Receiver, Snapshot, FRAME_BYTES};

const SKIPPED: u64 = 1 << 63;
#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut safety_receiver_header: [u64; 6] = [0x53465031, 0, 0, 0, 0, 0];
#[no_mangle]
static mut safety_activations: [[u64; 19]; 24] = [[0; 19]; 24];
#[no_mangle]
static mut safety_activation_count: usize = 0;
#[no_mangle]
static mut safety_received_rows: [[u64; 20]; 64] = [[0; 20]; 64];
// Active-case sidecar indexed by safety_received_rows. Each half is the private
// generation, enabled, faulted and kernel ticks, generation, flags, sequence, issue.
#[no_mangle]
static mut safety_active_received: [[u64; 16]; 64] = [[0; 16]; 64];
// Request, reply, info status, private generation, sequence, issue, permit,
// enabled, faulted, then kernel ticks, generation, flags, sequence and issue.
#[no_mangle]
static mut safety_active_ack: [u64; 14] = [0; 14];
#[no_mangle]
static mut safety_received_bytes: [[u8; FRAME_BYTES]; 64] = [[0; FRAME_BYTES]; 64];
#[no_mangle]
static mut safety_received_count: usize = 0;
#[no_mangle]
static mut safety_exact_clock_control: [u64; 12] = [0; 12];
#[no_mangle]
static mut safety_exact_rearm_control: [u64; 7] = [0; 7];
// Activations 8 and 9. Latched booleans are private-model check results;
// the final flags are the protected INFO readback after COMPLETE.
#[no_mangle]
static mut safety_phase_samples: [[u64; 12]; 2] = [[0; 12]; 2];

fn values(s: Snapshot) -> [u64; 4] {
    [
        s.sequence,
        s.issue.unwrap_or(0),
        s.permit as u64,
        s.recovery as u64,
    ]
}
fn kernel_values(s: &dev::State) -> [u64; 5] {
    [
        s.ticks,
        s.generation,
        s.flags,
        s.last_sequence,
        s.last_issue_ticks,
    ]
}
struct Controller {
    model: Receiver,
    time: Timeline,
    divergences: u64,
    active_ack: Option<bool>,
}
impl Controller {
    fn one(&mut self, iteration: usize, ordinal: usize, active: bool) -> Bench<i64> {
        let loan = match loan::receive() {
            Ok(r) => r,
            Err(e) => return Ok(e),
        };
        let handle = loan.handle;
        let len = loan.length;
        let slot = (handle & 3) as usize;
        let va = loan.address();
        let before_state = self.model.snapshot();
        let before = values(before_state);
        let mut bytes = [0; FRAME_BYTES];
        let mut checked = validate_loan(0, 1, slot, len);
        if va != 0x01600000 + slot * 4096 {
            checked = Err(Error::Slot)
        }
        if checked.is_ok() {
            // Trusted, configured 0->1 route on one core. The supervisor has
            // lower priority; exit barriers keep the other roles alive here.
            for (i, b) in bytes.iter_mut().enumerate() {
                *b = unsafe { loan.read_page_byte(i) };
            }
        }
        let released = status(loan::release(handle));
        if released != 0 {
            return Err(89);
        }
        let mut candidate = self.model;
        let mut decision = 0;
        let mut clock_error = false;
        let mut prior_kernel = None;
        let valid = checked.and_then(|()| {
            candidate.accept(&bytes, || match dev::info() {
                Ok(v) => {
                    decision = v.ticks;
                    if active {
                        prior_kernel = Some(kernel_values(&v));
                    }
                    decision
                }
                Err(_) => {
                    clock_error = true;
                    u64::MAX
                }
            })
        });
        if clock_error {
            return Err(82);
        }
        // A transport refusal can precede the decision clock. Capture the
        // kernel tuple before any commit in that case as well.
        if active && prior_kernel.is_none() {
            prior_kernel = Some(kernel_values(&state()?));
        }
        let local = valid.as_ref().err().map_or(0, |e| *e as u64);
        let mut kernel = SKIPPED;
        if valid.is_ok() {
            let v = candidate.snapshot();
            let rc = status(dev::commit(
                v.generation,
                v.sequence,
                v.issue.ok_or(90)?,
                v.permit,
            ));
            kernel = rc as u64;
            if rc == 0 {
                self.model = candidate
            } else {
                self.model.check(state()?.ticks).map_err(|_| 90)?;
            }
        } else {
            // A rejected sample may independently latch an expired old permit.
            self.model = candidate;
        }
        let end = state()?;
        let after_state = self.model.snapshot();
        let after = values(after_state);
        unsafe {
            let at = safety_received_count;
            if at >= 64 {
                return Err(87);
            }
            if let Some(prior) = prior_kernel {
                let mut row = [0; 16];
                row[..3].copy_from_slice(&[
                    before_state.generation,
                    before_state.enabled as u64,
                    before_state.faulted as u64,
                ]);
                row[3..8].copy_from_slice(&prior);
                row[8..11].copy_from_slice(&[
                    after_state.generation,
                    after_state.enabled as u64,
                    after_state.faulted as u64,
                ]);
                row[11..].copy_from_slice(&kernel_values(&end));
                write_volatile(&raw mut safety_active_received[at], row);
            }
            write_volatile(&raw mut safety_received_bytes[at], bytes);
            write_volatile(
                &raw mut safety_received_rows[at],
                [
                    iteration as u64,
                    ordinal as u64,
                    handle,
                    va as u64,
                    len as u64,
                    local,
                    kernel,
                    decision,
                    end.ticks,
                    end.generation,
                    end.flags,
                    end.last_sequence,
                    before[0],
                    before[1],
                    before[2],
                    before[3],
                    after[0],
                    after[1],
                    after[2],
                    after[3],
                ],
            );
            write_volatile(&raw mut safety_received_count, at + 1);
        }
        Ok(0)
    }
    fn exact_control(&self) -> Bench<()> {
        let snap = self.model.snapshot();
        if snap.sequence != 3 || !snap.enabled {
            return Ok(());
        }
        unsafe {
            if safety_exact_clock_control[0] != 0 {
                return Ok(());
            }
        }
        let mut staged = self.model;
        let issue = snap.issue.ok_or(90)?;
        let expiry = issue.checked_add(20 * (self.time.hz / 1000)).ok_or(83)?;
        let near = staged.begin_activation(expiry - 1).map_err(|_| 90)?;
        let mut worst = staged;
        let worst_tick = expiry - 1 + 5 * (self.time.hz / 1000);
        let worst_latch = worst.begin_activation(worst_tick).map_err(|_| 90)?;
        let at = staged.begin_activation(expiry).map_err(|_| 90)?;
        let b = encode(Permit {
            generation: snap.generation,
            sequence: 4,
            issue: expiry,
            permit: true,
        })
        .map_err(|_| 90)?;
        let e = staged.accept(&b, || expiry).err().map_or(0, |e| e as u64);
        let after = staged.snapshot();
        unsafe {
            write_volatile(
                &raw mut safety_exact_clock_control,
                [
                    1,
                    issue,
                    expiry,
                    near as u64,
                    at as u64,
                    e,
                    after.sequence,
                    after.enabled as u64,
                    after.faulted as u64,
                    worst_tick,
                    worst_latch as u64,
                    worst.snapshot().enabled as u64,
                ],
            );
        }
        Ok(())
    }
    fn exact_rearm(&self, now: u64) -> Bench<()> {
        let s = self.model.snapshot();
        if s.sequence != 2 || s.enabled || s.recovery != 2 || now > self.time.at(15)? {
            return Ok(());
        }
        unsafe {
            if safety_exact_rearm_control[0] != 0 {
                return Ok(());
            }
        }
        let mut early = self.model;
        let mut edge = self.model;
        let t15 = self.time.at(15)?;
        let t20 = self.time.at(20)?;
        early.begin_activation(t15).map_err(|_| 90)?;
        edge.begin_activation(t20).map_err(|_| 90)?;
        let a = early
            .rearm(2, s.generation, t15)
            .err()
            .map_or(0, |e| e as u64);
        let b = edge
            .rearm(2, s.generation, t20)
            .err()
            .map_or(0, |e| e as u64);
        unsafe {
            write_volatile(
                &raw mut safety_exact_rearm_control,
                [
                    1,
                    t15,
                    a,
                    early.snapshot().enabled as u64,
                    t20,
                    b,
                    edge.snapshot().enabled as u64,
                ],
            );
        }
        Ok(())
    }
    fn acknowledge_active(&mut self, case: Case, activation_complete: bool) -> Bench<()> {
        let (token, word) = ipc::receive().map_err(|_| 84)?;
        // The call may have blocked. Sample after receipt and check a copy of
        // the private state at that same tick without adding a model transition.
        let actual = dev::info();
        let private = self.model.snapshot();
        let mut at_reply = self.model;
        let expected_issue = self.time.at(20 + case.active_offset_ms());
        let reply = match &actual {
            Ok(s)
                if activation_complete
                    && word == common::LAST_SAMPLE
                    && private.generation == case.active_generation()
                    && s.generation == private.generation
                    && private.sequence == 3
                    && s.last_sequence == private.sequence
                    && private.issue == Some(s.last_issue_ticks)
                    && expected_issue == Ok(s.last_issue_ticks)
                    && private.permit
                    && private.enabled
                    && !private.faulted
                    && s.flags & (dev::GATE_OPEN | dev::ENABLED | dev::FAULT)
                        == (dev::GATE_OPEN | dev::ENABLED)
                    && at_reply.check(s.ticks) == Ok(false) =>
            {
                s.last_sequence
            }
            _ => u64::MAX,
        };
        let mut row = [SKIPPED; 14];
        row[..9].copy_from_slice(&[
            word,
            reply,
            actual.as_ref().err().copied().unwrap_or(0) as u64,
            private.generation,
            private.sequence,
            private.issue.unwrap_or(0),
            private.permit as u64,
            private.enabled as u64,
            private.faulted as u64,
        ]);
        if let Ok(s) = &actual {
            row[9..].copy_from_slice(&kernel_values(s));
        }
        unsafe {
            write_volatile(&raw mut safety_active_ack, row);
        }
        self.active_ack = Some(reply != u64::MAX);
        ipc::reply(token, reply).map_err(|_| 84)
    }
    fn activate(&mut self, case: Case, i: usize, phase_leads: [u64; 2]) -> Bench<()> {
        let grid = i as u64 * 5;
        let planned = match case {
            Case::Collision => self.time.at(grid)?,
            Case::Phase | Case::PhaseDecision | Case::PhaseComplete if grid >= 25 => {
                self.time.at(grid)? - 1
            }
            _ => self.time.at(grid + 1)?,
        };
        let phase_lead = match i {
            8 | 9 if case.measured_phase() => Some(phase_leads[i - 8]),
            _ => None,
        };
        let wake_target = planned.checked_sub(phase_lead.unwrap_or(0)).ok_or(83)?;
        let wake = self.time.until(wake_target)?;
        dev::check().map_err(|_| 91)?;
        let entry = state()?;
        let expired = self.model.begin_activation(entry.ticks).map_err(|_| 90)?;
        let opened = entry.generation != self.model.snapshot().generation;
        if opened {
            self.model
                .open(2, entry.generation, entry.ticks)
                .map_err(|_| 90)?;
        }
        let mut attempts = [SKIPPED; 2];
        for (n, r) in attempts.iter_mut().enumerate() {
            let code = self.one(i, n, case.active_case())?;
            *r = code as u64;
            if code != 0 {
                if !matches!(code, -11 | -32) {
                    return Err(92);
                }
                break;
            }
        }
        let command = state()?;
        let mut rearm_local = SKIPPED;
        let mut rearm_kernel = SKIPPED;
        if !opened && command.flags & dev::PENDING_REARM != 0 {
            let mut candidate = self.model;
            let res = candidate.rearm(2, command.generation, command.ticks);
            rearm_local = res.as_ref().err().map_or(0, |e| *e as u64);
            if res.is_ok() {
                rearm_kernel = status(dev::process_rearm()) as u64;
                if rearm_kernel == 0 {
                    self.model = candidate;
                }
            }
            // COMPLETE discards a one-shot command the private receiver declined.
        }
        let before_complete = state()?.ticks;
        let pre_latched = self.model.check(before_complete).map_err(|_| 90)?;
        dev::complete().map_err(|_| 93)?;
        let end = state()?;
        let post_latched = self.model.check(end.ticks).map_err(|_| 90)?;
        let after = self.model.snapshot();
        // The kernel's atomic completion stamp precedes this user observation.
        // A deadline falling in that interval is visible in the two raw clocks.
        if after.sequence != end.last_sequence || after.issue.unwrap_or(0) != end.last_issue_ticks {
            self.divergences += 1;
        }
        unsafe {
            write_volatile(
                &raw mut safety_activations[i],
                [
                    planned,
                    wake,
                    entry.ticks,
                    end.ticks,
                    entry.generation,
                    entry.flags,
                    expired as u64,
                    attempts[0],
                    attempts[1],
                    rearm_local,
                    rearm_kernel,
                    end.flags,
                    end.last_sequence,
                    end.last_issue_ticks,
                    after.sequence,
                    after.issue.unwrap_or(0),
                    after.recovery as u64,
                    after.enabled as u64,
                    after.faulted as u64,
                ],
            );
            write_volatile(&raw mut safety_activation_count, i + 1);
            if let Some(lead) = phase_lead {
                write_volatile(
                    &raw mut safety_phase_samples[i - 8],
                    [
                        i as u64,
                        planned,
                        lead,
                        wake_target,
                        wake,
                        entry.ticks,
                        expired as u64,
                        before_complete,
                        pre_latched as u64,
                        end.ticks,
                        post_latched as u64,
                        end.flags,
                    ],
                );
            }
        }
        if matches!(case, Case::Collision | Case::Phase) {
            self.exact_control()?;
        }
        if matches!(case, Case::Early | Case::Boundary) {
            self.exact_rearm(end.ticks)?;
        }
        if i == 4
            && matches!(
                case,
                Case::Silence | Case::Phase | Case::PhaseDecision | Case::PhaseComplete
            )
        {
            let (token, word) = ipc::receive().map_err(|_| 84)?;
            if word != common::LAST_SAMPLE {
                return Err(84);
            }
            ipc::reply(token, end.last_sequence).map_err(|_| 84)?;
        }
        if case.active_case() && i as u64 == 4 + case.active_offset_ms() / 5 {
            self.acknowledge_active(case, true)?;
        }
        Ok(())
    }
}
fn run(case: Case, phase_leads: [u64; 2]) -> Bench<()> {
    let time = Timeline::start(case)?;
    let mut c = Controller {
        model: Receiver::new(time.hz).map_err(|_| 90)?,
        time,
        divergences: 0,
        active_ack: None,
    };
    let mut result = Ok(());
    for i in 0..17 {
        if let Err(e) = c.activate(case, i, phase_leads) {
            result = Err(e);
            break;
        }
    }
    if case.active_case() && c.active_ack.is_none() {
        // An earlier activation failed before the supervisor's rendezvous.
        // Answer it before entering the normal two-message exit protocol.
        c.acknowledge_active(case, false)?;
    }
    if result.is_ok() && c.active_ack == Some(false) {
        result = Err(94);
    }
    unsafe {
        write_volatile(
            &raw mut safety_receiver_header,
            [
                0x53465031,
                case as u64,
                time.epoch,
                time.hz,
                c.divergences,
                result.as_ref().err().copied().unwrap_or(0) as u64,
            ],
        );
    }
    common::finish()?;
    result?;
    if c.divergences == 0 {
        Ok(())
    } else {
        Err(94)
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}
#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    (|| {
        if !matches!(argc, 2 | 4) {
            return Err(81);
        }
        let case = Case::parse(2, argv)?;
        let mut leads = [0; 2];
        if argc == 4 {
            if !case.measured_phase() {
                return Err(81);
            }
            leads[0] = common::parse_lead_ticks(*argv.add(2))?;
            leads[1] = common::parse_lead_ticks(*argv.add(3))?;
        }
        run(case, leads)
    })()
    .err()
    .unwrap_or(72)
}
