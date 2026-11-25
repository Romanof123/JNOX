#![no_std]
#![no_main]
mod controller;
#[allow(dead_code)]
#[path = "../../native-safety-admission/common.rs"]
mod schedule;
mod sender;
mod supervisor;
use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, ipc, safety_channel as dev};
use schedule::{info, Result, Time};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
enum Case {
    Healthy = 1,
    Sleep,
    Compute,
    Exit,
    Recover,
    RecoverLive,
}
impl Case {
    fn recovery(self) -> bool {
        matches!(self, Self::Recover | Self::RecoverLive)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
enum Role {
    Sender = 0,
    Controller = 1,
    Supervisor = 2,
}
unsafe fn arg<'a>(p: *const u8) -> Result<&'a [u8]> {
    if p.is_null() {
        return Err(81);
    }
    let n = (0..16).find(|&i| *p.add(i) == 0).ok_or(81)?;
    Ok(core::slice::from_raw_parts(p, n))
}
const STOP: u64 = 0x574453544f50;
fn ack(case: Case, role: Role) -> u64 {
    0x5744000000000000 | (case as u64) << 8 | role as u64
}
fn start(case: Case, role: Role) -> Result<Time> {
    let s = info()?;
    if role == Role::Supervisor {
        let epoch = Time {
            epoch: s.ticks,
            hz: s.hz,
        }
        .at(50_000)?;
        dev::supervise(dev::Command::Open, 1).map_err(|_| 83)?;
        for peer in [Role::Sender, Role::Controller] {
            if ipc::call(peer as u64, epoch, u64::MAX) != Ok(ack(case, peer)) {
                return Err(84);
            }
        }
        if info()?.ticks >= epoch {
            return Err(85);
        }
        Ok(Time { epoch, hz: s.hz })
    } else {
        let (token, epoch) = ipc::receive().map_err(|_| 84)?;
        ipc::reply(token, ack(case, role)).map_err(|_| 84)?;
        Ok(Time { epoch, hz: s.hz })
    }
}
fn park() -> Result<()> {
    let (token, word) = ipc::receive().map_err(|_| 84)?;
    if word != STOP {
        return Err(84);
    }
    ipc::reply(token, STOP).map_err(|_| 84)
}
fn stop(case: Case) -> Result<()> {
    for peer in [Role::Sender, Role::Controller] {
        let result = ipc::call(peer as u64, STOP, u64::MAX);
        if matches!(result, Err(-32) | Err(-3)) && peer == Role::Controller && case == Case::Exit {
            continue;
        }
        if result != Ok(STOP) {
            return Err(84);
        }
    }
    Ok(())
}
fn active(s: &dev::State, gen: u64) -> bool {
    let required = dev::GATE_OPEN | dev::ENABLED | dev::WATCHDOG_PRIMED;
    s.generation == gen
        && s.flags & required == required
        && s.flags & (dev::FAULT | dev::WATCHDOG_FAULT) == 0
}
fn inhibited(s: &dev::State) -> bool {
    s.flags & (dev::FAULT | dev::WATCHDOG_FAULT) == (dev::FAULT | dev::WATCHDOG_FAULT)
        && s.flags & (dev::ENABLED | dev::PENDING_REARM | dev::WATCHDOG_PRIMED) == 0
}
#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut wd_header: [u64; 6] = [0x57444231, 0, 0, 0, 0, 0];
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}
#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let result = (|| {
        if argc != 3 || argv.is_null() {
            return Err(81);
        }
        let role = match arg(*argv.add(1))? {
            b"sender" => Role::Sender,
            b"controller" => Role::Controller,
            b"supervisor" => Role::Supervisor,
            _ => return Err(81),
        };
        let case = match arg(*argv.add(2))? {
            b"healthy" => Case::Healthy,
            b"sleep-late" => Case::Sleep,
            b"compute-late" => Case::Compute,
            b"controller-exit" => Case::Exit,
            b"recover" => Case::Recover,
            b"recover-live" => Case::RecoverLive,
            _ => return Err(81),
        };
        let rounds = if role == Role::Controller && case == Case::Compute {
            controller::calibrate()?
        } else {
            0
        };
        let t = start(case, role)?;
        write_volatile(
            &raw mut wd_header,
            [0x57444231, role as u64, case as u64, t.epoch, t.hz, 0],
        );
        let r = match role {
            Role::Sender => sender::run(case, t),
            Role::Controller => controller::run(case, t, rounds),
            Role::Supervisor => supervisor::run(case, t),
        };
        write_volatile(&raw mut wd_header[5], r.err().unwrap_or(0) as u64);
        if role == Role::Supervisor {
            stop(case)?;
        } else {
            park()?;
        }
        r?;
        Ok(71 + role as i32)
    })();
    result.unwrap_or_else(|e| e)
}
