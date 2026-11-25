use core::ptr::write_volatile;
use jnox_native::{ipc, permit_trace, safety_channel as dev};

pub type Result<T> = core::result::Result<T, i32>;
pub const JOBS: usize = 16;
pub const SKIP: u64 = 1 << 63;
const ACK: u64 = 0x51414c0000000000;
const DONE: u64 = 0x444f4e45;
const STOP: u64 = 0x53544f50;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Case {
    Healthy = 1,
    Live,
    Silence,
    Invalid,
    Saturation,
    Collision,
    Delayed,
    BoundaryWitness,
    LowerWork,
    Overrun,
    GateInvalid,
    GateSaturation,
}
impl Case {
    pub fn closed_gate(self) -> bool {
        matches!(self, Self::GateInvalid | Self::GateSaturation)
    }

    pub unsafe fn parse(argc: i32, argv: *const *const u8) -> Result<Self> {
        if argc != 2 || argv.is_null() || (*argv.add(1)).is_null() {
            return Err(81);
        }
        let p = *argv.add(1);
        let n = (0..17).find(|&i| *p.add(i) == 0).ok_or(81)?;
        match core::slice::from_raw_parts(p, n) {
            b"healthy" => Ok(Self::Healthy),
            b"live" => Ok(Self::Live),
            b"silence" => Ok(Self::Silence),
            b"invalid" => Ok(Self::Invalid),
            b"saturation" => Ok(Self::Saturation),
            b"collision" => Ok(Self::Collision),
            b"delayed" => Ok(Self::Delayed),
            b"boundary-witness" => Ok(Self::BoundaryWitness),
            b"lower-work" => Ok(Self::LowerWork),
            b"overrun" => Ok(Self::Overrun),
            b"gate-invalid" => Ok(Self::GateInvalid),
            b"gate-saturation" => Ok(Self::GateSaturation),
            _ => Err(81),
        }
    }
}

#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut admission_header: [u64; 6] = [0x51414c31, 0, 0, 0, 0, 0];
pub fn header(case: Case, t: Time) {
    unsafe {
        write_volatile(
            &raw mut admission_header,
            [0x51414c31, case as u64, t.epoch, t.hz, 0, 0],
        );
    }
}
pub fn completed(n: usize) {
    unsafe {
        write_volatile(&raw mut admission_header[4], n as u64);
    }
}
pub fn record_result(r: Result<()>) {
    unsafe {
        write_volatile(&raw mut admission_header[5], r.err().unwrap_or(0) as u64);
    }
}
pub fn info() -> Result<dev::State> {
    dev::info().map_err(|_| 82)
}
pub fn rc<T>(r: core::result::Result<T, i64>) -> u64 {
    r.err().unwrap_or(0) as u64
}

#[derive(Clone, Copy)]
pub struct Time {
    pub epoch: u64,
    pub hz: u64,
}
impl Time {
    pub fn at(self, us: u64) -> Result<u64> {
        let n = (self.hz as u128) * (us as u128);
        if self.hz == 0 || n % 1_000_000 != 0 {
            return Err(83);
        }
        self.epoch
            .checked_add(u64::try_from(n / 1_000_000).map_err(|_| 83)?)
            .ok_or(83)
    }
    pub fn origin(case: Case) -> Result<Self> {
        let s = info()?;
        let t = Time {
            epoch: Time {
                epoch: s.ticks,
                hz: s.hz,
            }
            .at(50_000)?,
            hz: s.hz,
        };
        dev::supervise(dev::Command::Open, 1).map_err(|_| 84)?;
        for pid in [0, 1] {
            if ipc::call(pid, t.epoch, u64::MAX) != Ok(ACK | case as u64) {
                return Err(85);
            }
        }
        if info()?.ticks >= t.epoch {
            return Err(86);
        }
        Ok(t)
    }
    pub fn join(case: Case) -> Result<Self> {
        let (token, epoch) = ipc::receive().map_err(|_| 85)?;
        let s = info()?;
        let t = Self { epoch, hz: s.hz };
        t.at(80_000)?;
        ipc::reply(token, ACK | case as u64).map_err(|_| 85)?;
        Ok(t)
    }
}
#[derive(Clone, Copy)]
#[repr(u64)]
pub enum Phase {
    A = 1,
    B = 2,
    Sender = 3,
    Supervisor = 4,
}

pub struct Segment {
    pub job: usize,
    pub phase: Phase,
    pub target: u64,
}
impl Segment {
    fn mark(&self, stage: u64) -> Result<()> {
        permit_trace::mark(self.job as u64, self.phase as u64, stage, self.target).map_err(|_| 87)
    }
    pub fn start(&self) -> Result<()> {
        self.mark(1)?; // One absolute scheduler wait, including for the B continuation.
        self.mark(2)
    }
    pub fn finish(&self) -> Result<()> {
        self.mark(3)
    }
}
// Keep both grant endpoints alive through the final measured controller phase.
// Every normal worker is already parked here before the supervisor sends STOP.
pub fn park() -> Result<()> {
    for word in [DONE, STOP] {
        let (token, request) = ipc::receive().map_err(|_| 85)?;
        if request != word {
            return Err(85);
        }
        ipc::reply(token, word).map_err(|_| 85)?;
    }
    Ok(())
}
pub fn stop() -> Result<()> {
    for word in [DONE, STOP] {
        for pid in [0, 1] {
            if ipc::call(pid, word, u64::MAX) != Ok(word) {
                return Err(85);
            }
        }
    }
    Ok(())
}
