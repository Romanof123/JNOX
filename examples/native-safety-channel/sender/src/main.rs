#![no_std]
#![no_main]

#[allow(dead_code)]
#[path = "../../common.rs"]
mod common;
use common::{state, Bench, Case, Timeline};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, ipc, loan};
use jnox_safety_channel::{crc32, encode, Permit, FRAME_BYTES};

#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut safety_sender_header: [u64; 5] = [0x53465030, 0, 0, 0, 0];
#[no_mangle]
static mut safety_sender_rows: [[u64; 11]; 64] = [[0; 11]; 64];
#[no_mangle]
static mut safety_sender_bytes: [[u8; FRAME_BYTES]; 64] = [[0; FRAME_BYTES]; 64];
#[no_mangle]
static mut safety_sender_count: usize = 0;

const ACTIVE_INPUT: u64 = 10;

#[derive(Clone, Copy)]
struct Message {
    gen: u64,
    seq: u64,
    issue: u64,
    set: bool,
    kind: u64,
}
impl Message {
    fn bytes(self, case: Case) -> Bench<[u8; FRAME_BYTES]> {
        let mut b = encode(Permit {
            generation: self.gen,
            sequence: self.seq,
            issue: self.issue,
            permit: self.set,
        })
        .map_err(|_| 86)?;
        match self.kind {
            1 => b[40] ^= 1, // checksum test vector
            2 | 6 => {
                if self.kind == 2 {
                    b[6..8].copy_from_slice(&0x0102u16.to_le_bytes())
                } else {
                    b[4..6].copy_from_slice(&2u16.to_le_bytes())
                }
                let checksum = crc32(&b[..40]).to_le_bytes();
                b[40..44].copy_from_slice(&checksum);
            }
            ACTIVE_INPUT => {
                match case {
                    // Each checksum case changes one byte after encoding.
                    Case::BadCrc => {
                        b[36] = 0;
                        return Ok(b);
                    }
                    Case::BadHeaderCrc => {
                        b[0] ^= 1;
                        return Ok(b);
                    }
                    Case::BadLength => b[2..4].copy_from_slice(&43u16.to_le_bytes()),
                    Case::BadVersion => b[0] = 2,
                    Case::BadType => b[1] = 2,
                    Case::BadChannel => b[4..6].copy_from_slice(&2u16.to_le_bytes()),
                    Case::BadDataId => b[6..8].copy_from_slice(&0x0102u16.to_le_bytes()),
                    Case::BadValidity => b[32..36].copy_from_slice(&19u32.to_le_bytes()),
                    Case::BadPermit => b[36] = 2,
                    Case::BadReserved => b[37] = 1,
                    // These cases encoded their single changed field in Message.
                    Case::Duplicate
                    | Case::Reordered
                    | Case::Gap
                    | Case::OldGeneration
                    | Case::FutureIssue
                    | Case::ExpiredIssue
                    | Case::IssueOrder => return Ok(b),
                    _ => return Err(86),
                }
                let checksum = crc32(&b[..40]).to_le_bytes();
                b[40..44].copy_from_slice(&checksum);
            }
            _ => {}
        }
        Ok(b)
    }
}
struct Sender {
    time: Timeline,
    case: Case,
    unexpected: u64,
}
impl Sender {
    fn msg(&self, gen: u64, seq: u64, ms: u64, set: bool, kind: u64) -> Bench<Message> {
        Ok(Message {
            gen,
            seq,
            issue: self.time.at(ms)?,
            set,
            kind,
        })
    }
    // mode 0 acquires and publishes, mode 1 stages a writer, mode 2 retries a
    // prior handle. Retry never touches the potentially revoked pointer.
    fn send(&mut self, mut m: Message, ms: u64, mode: u64, old: u64, expected: i64) -> Bench<u64> {
        let target = self.time.at(ms)?;
        let start = self.time.until(target)?;
        if m.kind == 9 && mode != 2 {
            m.issue = start;
        }
        let bytes = m.bytes(self.case)?;
        let (handle, acquired, published) = if mode == 2 {
            (old, 0, loan::publish(old, FRAME_BYTES).err().unwrap_or(0))
        } else {
            match loan::acquire() {
                Ok(w) => {
                    for (offset, &byte) in bytes.iter().enumerate() {
                        // This fixed, single-core sender outranks the OPEN/CLOSE
                        // issuer. It makes no blocking call while filling a grant.
                        unsafe { w.write_byte(offset, byte) }
                    }
                    let rc = if mode == 1 {
                        0
                    } else {
                        loan::publish(w.handle, FRAME_BYTES).err().unwrap_or(0)
                    };
                    (w.handle, 0, rc)
                }
                Err(e) => (0, e, 0),
            }
        };
        let end = state()?.ticks;
        let actual = if acquired != 0 { acquired } else { published };
        if actual != expected {
            self.unexpected += 1;
        }
        unsafe {
            let i = safety_sender_count;
            if i >= 64 {
                return Err(87);
            }
            write_volatile(
                &raw mut safety_sender_rows[i],
                [
                    target,
                    start,
                    end,
                    mode,
                    m.kind,
                    m.gen,
                    m.seq,
                    m.issue,
                    handle,
                    acquired as u64,
                    published as u64,
                ],
            );
            write_volatile(&raw mut safety_sender_bytes[i], bytes);
            write_volatile(&raw mut safety_sender_count, i + 1);
        }
        if published == -11 && expected != -11 {
            let _ = loan::release(handle);
        }
        Ok(handle)
    }
    fn normal(&mut self, gen: u64, seq: u64, ms: u64, set: bool) -> Bench<()> {
        let live = self.case == Case::LiveStartup || (self.case == Case::Observed && seq >= 3);
        self.send(
            self.msg(gen, seq, ms, set, if live { 9 } else { 0 })?,
            ms,
            0,
            0,
            0,
        )?;
        Ok(())
    }

    fn prime(&mut self, generation: u64, offset: u64) -> Bench<()> {
        for (seq, ms) in [(1, 0), (2, 10), (3, 20)] {
            self.normal(generation, seq, offset + ms, true)?;
        }
        Ok(())
    }

    fn active(&mut self, case: Case) -> Bench<()> {
        let offset = case.active_offset_ms();
        let generation = case.active_generation();
        if case == Case::OldGeneration {
            self.prime(1, 0)?;
        }
        self.prime(generation, offset)?;
        acknowledge(common::RELEASE_INPUT, true)?;
        if case == Case::WrongOwner {
            let staged = self.msg(generation, 4, offset + 24, true, 0)?;
            let handle = self.send(staged, offset + 24, 1, 0, 0)?;
            // IPC exchanges public synchronization words. PID0 alone holds
            // the writer and publishes its original bytes after the check.
            acknowledge(common::GRANT_STAGED, handle != 0)?;
            acknowledge(common::PUBLISH_GRANT, true)?;
            self.send(staged, offset + 30, 2, handle, 0)?;
        } else {
            let seq = match case {
                Case::Duplicate => 3,
                Case::Reordered => 2,
                Case::Gap => 5,
                _ => 4,
            };
            let issue = offset
                + match case {
                    Case::FutureIssue => 45,
                    Case::ExpiredIssue => 0,
                    Case::IssueOrder => 20,
                    _ => 24,
                };
            let wire_generation = if case == Case::OldGeneration {
                1
            } else {
                generation
            };
            let frame = self.msg(wire_generation, seq, issue, true, ACTIVE_INPUT)?;
            self.send(frame, offset + 24, 0, 0, 0)?;
            self.normal(generation, 4, offset + 30, true)?;
        }
        self.normal(generation, 5, offset + 36, false)
    }

    fn run(&mut self, case: Case) -> Bench<()> {
        if case.active_case() {
            return self.active(case);
        }
        if case == Case::Delayed {
            self.send(self.msg(1, 1, 0, true, 7)?, 25, 0, 0, 0)?;
            self.normal(1, 1, 30, true)?;
            self.normal(1, 2, 40, true)?;
            self.normal(1, 3, 50, true)?;
            self.normal(1, 4, 60, false)?;
            return Ok(());
        }
        self.normal(1, 1, 0, true)?;
        if case == Case::One {
            return Ok(());
        }
        self.normal(1, 2, if case == Case::BadSpacing { 11 } else { 10 }, true)?;
        if case.rearm_target_ms().is_some()
            || matches!(
                case,
                Case::Early | Case::Boundary | Case::NoRearm | Case::BadSpacing
            )
        {
            return Ok(());
        }
        self.normal(1, 3, 20, true)?;
        match case {
            Case::Healthy | Case::Observed | Case::LiveStartup => {
                for (seq, ms) in [(4, 30), (5, 40), (6, 50)] {
                    self.normal(1, seq, ms, true)?;
                }
                self.normal(1, 7, 60, false)?;
            }
            Case::Silence | Case::Phase => {
                self.send(self.msg(1, 4, 30, true, 0)?, 30, 0, 0, -32)?;
            }
            Case::Queued => {
                self.normal(1, 4, 22, true)?;
                let mut next = self.msg(1, 5, 22, true, 0)?;
                next.issue += 1;
                self.send(next, 22, 0, 0, 0)?;
                let m = self.msg(1, 6, 22, true, 0)?;
                let held = self.send(m, 22, 0, 0, -11)?;
                self.send(m, 30, 2, held, -32)?;
            }
            Case::Invalid => {
                for (send, kind, seq, issue) in [
                    (24, 1, 4, 24),
                    (28, 2, 4, 28),
                    (32, 3, 4, 50),
                    (34, 4, 5, 34),
                    (37, 5, 3, 37),
                    (39, 6, 4, 39),
                    (43, 1, 4, 43),
                    (48, 2, 4, 48),
                    (53, 1, 4, 53),
                    (58, 2, 4, 58),
                ] {
                    self.send(self.msg(1, seq, issue, true, kind)?, send, 0, 0, 0)?;
                }
            }
            Case::Saturation => {
                let m = self.msg(1, 4, 24, true, 1)?;
                self.send(m, 24, 0, 0, 0)?;
                self.send(m, 24, 0, 0, 0)?;
                let h = self.send(m, 24, 0, 0, -11)?;
                self.send(m, 28, 2, h, 0)?;
                for ms in [30, 34, 39, 44, 49, 54, 59] {
                    self.send(self.msg(1, 4, ms, true, 1)?, ms, 0, 0, 0)?;
                }
            }
            Case::Collision => {
                self.normal(1, 4, 40, true)?;
            }
            Case::Restart => {
                self.normal(1, 4, 30, false)?;
                let old = self.msg(1, 5, 33, true, 0)?;
                let held = self.send(old, 33, 1, 0, 0)?;
                self.send(old, 42, 2, held, -116)?;
                self.send(self.msg(1, 1, 43, true, 8)?, 43, 0, 0, 0)?;
                self.normal(2, 1, 45, true)?;
                self.normal(2, 2, 55, true)?;
                self.normal(2, 3, 65, true)?;
                self.normal(2, 4, 75, false)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn acknowledge(expected: u64, ready: bool) -> Bench<()> {
    let (token, word) = ipc::receive().map_err(|_| 84)?;
    let accepted = ready && word == expected;
    ipc::reply(token, if accepted { word } else { 0 }).map_err(|_| 84)?;
    if accepted {
        Ok(())
    } else {
        Err(84)
    }
}

fn run(case: Case) -> Bench<()> {
    let time = Timeline::start(case)?;
    let mut sender = Sender {
        time,
        case,
        unexpected: 0,
    };
    let result = sender.run(case);
    unsafe {
        write_volatile(
            &raw mut safety_sender_header,
            [
                0x53465030,
                case as u64,
                time.epoch,
                time.hz,
                sender.unexpected,
            ],
        );
    }
    common::finish()?;
    result?;
    if sender.unexpected == 0 {
        Ok(())
    } else {
        Err(88)
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}
#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    Case::parse(argc, argv).and_then(run).err().unwrap_or(71)
}
