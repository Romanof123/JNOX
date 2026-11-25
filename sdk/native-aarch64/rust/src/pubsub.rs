//! Typed publish/subscribe calls to an explicitly selected native broker.
use core::mem::MaybeUninit;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription { pub slot: u64, pub generation: u64 }

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    pub queued: u8,
    pub high_water: u8,
    pub lost: u64,
    pub front_sequence: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub sequence: u64,
    pub length: u8,
    pub data: [u8; 32],
}
impl Sample {
    pub fn bytes(&self) -> &[u8] { &self.data[..usize::from(self.length)] }
}

extern "C" {
    fn jnox_subscribe(broker: u64, topic: u8, out: *mut Subscription) -> i64;
    fn jnox_unsubscribe(broker: u64, sub: Subscription) -> i64;
    fn jnox_publish(broker: u64, topic: u8, p: *const u8, n: usize, out: *mut u64) -> i64;
    fn jnox_observe(broker: u64, sub: Subscription, out: *mut Observation) -> i64;
    fn jnox_peek(broker: u64, sub: Subscription, out: *mut Sample) -> i64;
    fn jnox_receive(broker: u64, sub: Subscription, wait: i32, out: *mut Sample) -> i64;
}

// The C wrappers initialize every output field on success and do not
// initialize outputs on error. Only those wrappers may be passed here.
unsafe fn output<T>(call: impl FnOnce(*mut T) -> i64) -> Result<T, i64> {
    let mut out = MaybeUninit::uninit();
    let status = call(out.as_mut_ptr());
    if status != 0 { return Err(status); }
    Ok(unsafe { out.assume_init() })
}

pub fn subscribe(broker: u64, topic: u8) -> Result<Subscription, i64> {
    unsafe { output(|out| jnox_subscribe(broker, topic, out)) }
}
pub fn unsubscribe(broker: u64, sub: Subscription) -> Result<(), i64> {
    let status = unsafe { jnox_unsubscribe(broker, sub) };
    if status == 0 { Ok(()) } else { Err(status) }
}
pub fn publish(broker: u64, topic: u8, data: &[u8]) -> Result<u64, i64> {
    unsafe { output(|out| jnox_publish(broker, topic, data.as_ptr(), data.len(), out)) }
}
pub fn observe(broker: u64, sub: Subscription) -> Result<Observation, i64> {
    unsafe { output(|out| jnox_observe(broker, sub, out)) }
}
pub fn peek(broker: u64, sub: Subscription) -> Result<Sample, i64> {
    unsafe { output(|out| jnox_peek(broker, sub, out)) }
}
/// Nonblocking empty receives return -11. A waiting receiver drains queued
/// records before receiving -32 after the last authorized publisher exits.
pub fn receive(broker: u64, sub: Subscription, wait: bool) -> Result<Sample, i64> {
    unsafe { output(|out| jnox_receive(broker, sub, i32::from(wait), out)) }
}
