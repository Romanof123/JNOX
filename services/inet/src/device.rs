//! Single-frame userspace device. An EAGAIN transmit retains the frame until the next poll.
use core::arch::asm;
use smoltcp::{
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
};

pub const MTU: usize = 1514;

pub struct Frames {
    rx: [u8; MTU],
    tx: Tx,
    pub error: bool,
}
impl Frames {
    pub const EMPTY: Self = Self {
        rx: [0; MTU],
        tx: Tx {
            bytes: [0; MTU],
            len: 0,
            error: false,
            udp_ports: [0; 8],
            udp_count: 0,
        },
        error: false,
    };
    pub fn flush(&mut self) {
        self.tx.flush();
        self.error |= self.tx.error;
    }
    pub fn has_pending(&self) -> bool {
        self.tx.len != 0
    }
    pub fn take_udp_port(&mut self) -> Option<u16> {
        self.tx.udp_count = self.tx.udp_count.checked_sub(1)?;
        Some(self.tx.udp_ports[self.tx.udp_count])
    }
}

pub struct Tx {
    bytes: [u8; MTU],
    len: usize,
    error: bool,
    udp_ports: [u16; 8],
    udp_count: usize,
}
impl Tx {
    fn flush(&mut self) {
        if self.len == 0 || self.error {
            return;
        }
        let n = frame(39, self.bytes.as_ptr() as usize, self.len);
        if n == self.len as i64 {
            // Count actual device acceptance, including a zero-length datagram.
            // This lets close drain committed outbound UDP records before reuse.
            let b = &self.bytes[..self.len];
            if b.len() >= 42 && b[12..14] == [8, 0] && b[23] == 17 {
                let start = 14 + 4 * usize::from(b[14] & 15);
                if start + 8 <= b.len() {
                    self.udp_ports[self.udp_count] = u16::from_be_bytes([b[start], b[start + 1]]);
                    self.udp_count += 1;
                }
            }
            self.len = 0;
        } else if n != -11 {
            self.error = true;
        }
    }
}
fn frame(op: u64, address: usize, len: usize) -> i64 {
    let mut status = address as i64;
    unsafe {
        // The dispatcher clears x1 on return. The caller can still need the
        // original length to distinguish a complete frame from a short write.
        asm!("svc #0x4a01", in("x8") op, inout("x0") status,
             inlateout("x1") len => _, options(nostack));
    }
    status
}
pub struct In<'a>(&'a [u8]);
impl RxToken for In<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(self.0)
    }
}
pub struct Out<'a>(&'a mut Tx);
impl TxToken for Out<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let tx = self.0;
        let result = f(&mut tx.bytes[..len]);
        tx.len = len;
        tx.flush();
        result
    }
}
impl Device for Frames {
    type RxToken<'a>
        = In<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = Out<'a>
    where
        Self: 'a;
    fn receive(&mut self, _: Instant) -> Option<(In<'_>, Out<'_>)> {
        if self.error || self.tx.error || self.has_pending() {
            return None;
        }
        match frame(38, self.rx.as_mut_ptr() as usize, MTU) {
            -11 => None,
            n if (14..=MTU as i64).contains(&n) => {
                Some((In(&self.rx[..n as usize]), Out(&mut self.tx)))
            }
            _ => {
                self.error = true;
                None
            }
        }
    }
    fn transmit(&mut self, _: Instant) -> Option<Out<'_>> {
        if self.error || self.tx.error || self.has_pending() {
            None
        } else {
            Some(Out(&mut self.tx))
        }
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = MTU;
        caps.max_burst_size = Some(1);
        caps
    }
}
