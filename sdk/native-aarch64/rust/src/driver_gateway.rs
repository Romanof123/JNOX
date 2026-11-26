//! Fixed-size diagnostic exchange with an assigned Linux device gateway.
//! Each gateway has a single caller and one outstanding request.
//! The package grants a route to its IPv4 broker or a Bao shared-memory copy
//! channel. Timeout and reply admission use the native monotonic clock.
#[cfg(feature = "local-socket")]
use crate::local_socket as s;
use crate::{monotonic_time, sleep_until, ClockError};
use core::time::Duration;

const RECORD_BYTES: usize = 24;
const MAX_TIMEOUT_MS: u32 = 10_000;
const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy)]
pub struct Request {
    pub generation: u32,
    pub id: u32,
    pub timeout_ms: u32,
    pub payload: [u8; 8],
}

impl Request {
    fn encode(self) -> [u8; RECORD_BYTES] {
        let mut record = [0; RECORD_BYTES];
        record[..4].copy_from_slice(b"JG\x01\x01");
        record[4..8].copy_from_slice(&self.generation.to_be_bytes());
        record[8..12].copy_from_slice(&self.id.to_be_bytes());
        record[12..16].copy_from_slice(&self.timeout_ms.to_be_bytes());
        record[16..].copy_from_slice(&self.payload);
        record
    }
}

pub enum Outcome {
    Reply([u8; 8]),
    TimedOut,
}

pub struct Receipt {
    pub outcome: Outcome,
    pub started: Duration,
    pub deadline: Duration,
    /// Native time when the response was accepted or expiry was observed.
    pub finished: Duration,
    pub stale_replies: u32,
    pub last_stale: Option<Duration>,
    pub invalid_replies: u32,
}

fn clock_error(e: ClockError) -> i32 {
    match e {
        ClockError::Errno(code) => -code,
        ClockError::InvalidTimespec => -5,
    }
}

fn now() -> Result<Duration, i32> {
    monotonic_time().map_err(clock_error)
}

fn wait(now: Duration, deadline: Duration) -> Result<(), i32> {
    if now < deadline {
        sleep_until(now.saturating_add(POLL_INTERVAL).min(deadline)).map_err(clock_error)?;
    }
    Ok(())
}

/// One kernel-mediated, fixed-size copy per call. The package grants the slot.
#[cfg(feature = "bao-gateway")]
pub mod bao {
    use core::arch::asm;

    fn copy(op: u64, address: usize) -> Result<(), i32> {
        let status: i64;
        // Both buffers are borrowed for this synchronous kernel copy.
        unsafe {
            asm!("svc #0x4a01", in("x8") op, inlateout("x0") address => status,
                 inlateout("x1") super::RECORD_BYTES => _, options(nostack));
        }
        match status {
            0 => Ok(()),
            n if n < 0 => Err(i32::try_from(n).unwrap_or(-5)),
            _ => Err(-5),
        }
    }

    pub fn send(record: &[u8; 24]) -> Result<(), i32> {
        copy(150, record.as_ptr() as usize)
    }

    pub fn receive(record: &mut [u8; 24]) -> Result<(), i32> {
        copy(151, record.as_mut_ptr() as usize)
    }
}

enum Transport {
    #[cfg(feature = "local-socket")]
    Udp { fd: i32, peer: s::Ipv4Endpoint },
    #[cfg(feature = "bao-gateway")]
    Bao,
}

pub struct Gateway {
    transport: Transport,
    last_request: Option<(u32, u32)>,
}

impl Gateway {
    #[cfg(feature = "local-socket")]
    pub fn bind(broker: u64, local: s::Ipv4Endpoint, peer: s::Ipv4Endpoint) -> Result<Self, i32> {
        s::set_broker(broker)?;
        let fd = s::socket4(s::SOCK_DGRAM, true)?;
        let gateway = Self {
            transport: Transport::Udp { fd, peer },
            last_request: None,
        };
        s::bind4(fd, local)?;
        Ok(gateway)
    }

    /// Use the address-free Bao copy channel selected by the native package.
    #[cfg(feature = "bao-gateway")]
    pub fn bao() -> Self {
        Self { transport: Transport::Bao, last_request: None }
    }

    fn send(&self, record: &[u8; RECORD_BYTES]) -> Result<usize, i32> {
        match self.transport {
            #[cfg(feature = "local-socket")]
            Transport::Udp { fd, peer } => s::send_to4(fd, record, s::MSG_DONTWAIT, peer),
            #[cfg(feature = "bao-gateway")]
            Transport::Bao => bao::send(record).map(|()| RECORD_BYTES),
        }
    }

    fn receive(&self, buf: &mut [u8; 32]) -> Result<(usize, bool), i32> {
        match self.transport {
            #[cfg(feature = "local-socket")]
            Transport::Udp { fd, peer } => s::recv_from4(fd, buf, s::MSG_DONTWAIT)
                .map(|(size, sender)| (size, sender == Some(peer))),
            #[cfg(feature = "bao-gateway")]
            Transport::Bao => {
                let record: &mut [u8; RECORD_BYTES] = (&mut buf[..RECORD_BYTES]).try_into().unwrap();
                bao::receive(record)?;
                Ok((RECORD_BYTES, true))
            }
        }
    }

    /// A new generation starts after Linux restarts. Never reuse a (generation,
    /// id) pair on this gateway, even if the earlier exchange timed out.
    pub fn exchange(&mut self, request: Request) -> Result<Receipt, i32> {
        let key = (request.generation, request.id);
        if request.generation == 0
            || request.id == 0
            || !(1..=MAX_TIMEOUT_MS).contains(&request.timeout_ms)
            || self.last_request.is_some_and(|previous| key <= previous)
        {
            return Err(-22);
        }
        let started = now()?;
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(request.timeout_ms)))
            .ok_or(-75)?;
        self.last_request = Some(key);
        let mut receipt = Receipt {
            outcome: Outcome::TimedOut,
            started,
            deadline,
            finished: started,
            stale_replies: 0,
            last_stale: None,
            invalid_replies: 0,
        };
        let record = request.encode();
        let mut expected = record;
        expected[3] = 2;
        for byte in &mut expected[16..] {
            *byte ^= 0xa5;
        }

        // Backpressure must not convert a bounded remote call into an unlimited send.
        while now()? < deadline {
            match self.send(&record) {
                Ok(RECORD_BYTES) => break,
                Ok(_) => return Err(-5),
                Err(-11) => wait(now()?, deadline)?,
                Err(code) => return Err(code),
            }
        }
        loop {
            receipt.finished = now()?;
            if receipt.finished >= deadline {
                return Ok(receipt);
            }
            // The broker admits datagrams up to 32 bytes. Read the full datagram
            // so an oversized record with a valid 24-byte prefix cannot match.
            let mut buf = [0; 32];
            match self.receive(&mut buf) {
                Err(-11) => (),
                Err(code) => return Err(code),
                Ok((size, peer_matches)) => {
                    receipt.finished = now()?;
                    if receipt.finished >= deadline {
                        return Ok(receipt);
                    }
                    if !peer_matches
                        || size != RECORD_BYTES
                        || buf[..4] != expected[..4]
                        || buf[8..RECORD_BYTES] != expected[8..]
                    {
                        receipt.invalid_replies = receipt.invalid_replies.saturating_add(1);
                    } else {
                        let generation = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
                        if generation == request.generation {
                            let mut payload = [0; 8];
                            payload.copy_from_slice(&buf[16..RECORD_BYTES]);
                            // The deadline applies to admission of the validated
                            // reply. Validation itself may have been preempted.
                            receipt.finished = now()?;
                            if receipt.finished >= deadline {
                                return Ok(receipt);
                            }
                            receipt.outcome = Outcome::Reply(payload);
                            return Ok(receipt);
                        } else if generation > 0 && generation < request.generation {
                            receipt.stale_replies = receipt.stale_replies.saturating_add(1);
                            receipt.last_stale = Some(receipt.finished);
                        } else {
                            receipt.invalid_replies = receipt.invalid_replies.saturating_add(1);
                        }
                    }
                }
            }
            wait(now()?, deadline)?;
        }
    }

    pub fn close(self) -> Result<(), i32> {
        match core::mem::ManuallyDrop::new(self).transport {
            #[cfg(feature = "local-socket")]
            Transport::Udp { fd, .. } => s::close(fd),
            #[cfg(feature = "bao-gateway")]
            Transport::Bao => Ok(()),
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        match self.transport {
            #[cfg(feature = "local-socket")]
            Transport::Udp { fd, .. } => { let _ = s::close(fd); }
            #[cfg(feature = "bao-gateway")]
            Transport::Bao => {}
        }
    }
}
