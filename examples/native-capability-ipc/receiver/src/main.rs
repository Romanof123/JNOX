#![no_std]
#![no_main]

use cap::{Handle, DELEGATE, SEND};
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{capabilities as cap, exit, ipc, mailbox};

#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut cap_receiver_magic: u64 = 0x43415052;
#[no_mangle]
static mut cap_receiver_log: [[u64; 3]; 40] = [[0; 3]; 40];
#[no_mangle]
static mut cap_receiver_nlog: usize = 0;
#[no_mangle]
static mut cap_receiver_notices: [[u64; 5]; 6] = [[0; 5]; 6];
#[no_mangle]
static mut cap_receiver_nnotices: usize = 0;
#[no_mangle]
static mut cap_receiver_counts: [[u64; 5]; 12] = [[0; 5]; 12];
#[no_mangle]
static mut cap_receiver_ncounts: usize = 0;

fn tag(n: u64) -> u64 {
    0xe910000000000000 | n
}
fn sample(n: u64) -> u64 {
    0xcafe000000000000 | (n << 32) | (0x301 + 11 * n)
}
fn ack(cmd: u64) -> u64 {
    0xace0000000000000 | cmd
}
type Check<T> = Result<T, i32>;

fn check(id: u64, actual: i64, wanted: i64, detail: u64) -> Check<()> {
    unsafe {
        let n = cap_receiver_nlog;
        if n >= 40 {
            return Err(110);
        }
        write_volatile(&raw mut cap_receiver_log[n], [id, actual as u64, detail]);
        write_volatile(&raw mut cap_receiver_nlog, n + 1);
    }
    if actual == wanted {
        Ok(())
    } else {
        Err(111)
    }
}
fn code<T>(value: Result<T, i64>) -> i64 {
    value.err().unwrap_or(0)
}
fn counts(phase: u64, handles: u64, notices: u64) -> Check<()> {
    let v = cap::observe().map_err(|_| 112)?;
    unsafe {
        let n = cap_receiver_ncounts;
        if n >= 12 {
            return Err(112);
        }
        write_volatile(
            &raw mut cap_receiver_counts[n],
            [phase, v.handles, v.notices, v.peak_handles, v.peak_notices],
        );
        write_volatile(&raw mut cap_receiver_ncounts, n + 1);
    }
    if (v.handles, v.notices) == (handles, notices) {
        Ok(())
    } else {
        Err(113)
    }
}
fn notice(n: u64, rights: u64) -> Check<Handle> {
    let got = cap::receive().map_err(|_| 114)?;
    unsafe {
        let i = cap_receiver_nnotices;
        if i >= 6 {
            return Err(114);
        }
        write_volatile(
            &raw mut cap_receiver_notices[i],
            [
                got.tag,
                got.handle.raw(),
                got.sender,
                got.target,
                got.rights,
            ],
        );
        write_volatile(&raw mut cap_receiver_nnotices, i + 1);
    }
    if got.tag != tag(n) || got.sender != 0 || got.target != 2 || got.rights != rights {
        return Err(115);
    }
    Ok(got.handle)
}
fn send(h: &Handle, n: u64) -> Check<()> {
    check(60 + n, code(cap::send(h, sample(n), 0)), 0, sample(n))
}
fn service() -> Check<()> {
    let mut slots: [Option<Handle>; 4] = [const { None }; 4];
    let mut released = None;
    loop {
        let (token, cmd) = ipc::receive().map_err(|_| 116)?;
        match cmd {
            1 => {
                // First two committed notices survived third, refused transfer.
                counts(1, 2, 2)?;
                slots[0] = Some(notice(0, SEND)?);
                slots[1] = Some(notice(1, DELEGATE)?);
                check(10, code(cap::receive()), -11, 0)?;
                check(11, code(mailbox::send(2, sample(99), 0)), -13, 2)?;
                let h = slots[0].as_ref().ok_or(117)?;
                check(12, code(cap::copy(h, 0, SEND, tag(93))), -13, h.raw())?;
                let delegate_only = slots[1].as_ref().ok_or(117)?;
                check(
                    20,
                    code(cap::send(delegate_only, sample(99), 0)),
                    -13,
                    delegate_only.raw(),
                )?;
                check(
                    21,
                    code(cap::copy(delegate_only, 0, SEND, tag(95))),
                    -13,
                    delegate_only.raw(),
                )?;
                send(h, 2)?;
                counts(2, 2, 0)?;
            }
            2 => {
                counts(3, 4, 2)?;
                slots[2] = Some(notice(2, SEND)?);
                slots[3] = Some(notice(3, SEND)?);
                counts(4, 4, 0)?;
            }
            3 => {
                // Failed MOVE into a table with no free handles and no pending notices.
                counts(5, 4, 0)?;
                let old = slots[0].take().ok_or(117)?;
                let raw = old.raw();
                released = Some(raw);
                check(13, code(cap::close(old)), 0, raw)?;
                check(
                    14,
                    code(cap::send(&Handle::from_raw(raw), sample(99), 0)),
                    -116,
                    raw,
                )?;
                counts(6, 3, 0)?;
            }
            4 => {
                counts(7, 4, 1)?;
                let h = notice(4, SEND | DELEGATE)?;
                let old = released.ok_or(117)?;
                // The kernel handle's low two bits select the owner-local slot.
                if old == h.raw() || old & 3 != h.raw() & 3 {
                    return Err(118);
                }
                check(15, code(cap::close(Handle::from_raw(old))), -116, old)?;
                send(&h, 4)?;
                check(16, code(cap::copy(&h, 0, SEND, tag(5))), 0, tag(5))?;
                slots[0] = Some(h);
            }
            5 => {
                send(slots[0].as_ref().ok_or(117)?, 6)?;
                for (i, slot) in slots.iter_mut().enumerate() {
                    let h = slot.take().ok_or(117)?;
                    let raw = h.raw();
                    check(30 + i as u64, code(cap::close(h)), 0, raw)?;
                }
                check(17, code(cap::receive()), -11, 0)?;
                counts(8, 0, 0)?;
            }
            6 => {
                counts(10, 1, 1)?;
                return Ok(());
            } // Exit while a notice is pending.
            7 => {
                let h = notice(6, SEND)?;
                send(&h, 8)?;
                counts(11, 1, 0)?;
                return Ok(()); // Exit while holding the taken capability.
            }
            8 => {
                slots[0] = Some(notice(0, SEND)?);
                counts(12, 1, 0)?;
            }
            9 => {
                let h = slots[0].as_ref().ok_or(117)?;
                check(18, code(cap::send(h, sample(99), 0)), -32, h.raw())?;
                check(19, code(cap::receive()), -11, 0)?;
                counts(13, 0, 0)?;
            }
            255 => return Ok(()),
            _ => return Err(119),
        }
        ipc::reply(token, ack(cmd)).map_err(|_| 120)?;
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(121)
}
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    service().err().unwrap_or(72)
}
