//! A fixed-size local drawing record and a six-byte directed input record.
use jnox_native::local_socket as socket;

pub const PATH: &[u8] = b"/windows";
pub const PROTOCOL_ERROR: i32 = 90;
pub const MASK_WIDTH: usize = 16;
pub const MASK_HEIGHT: usize = 12;
pub const SCALE: usize = 4;
const HEADER: usize = 6;
const DRAWING_BYTES: usize = HEADER + 8 + 2 * MASK_HEIGHT;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Id {
    A,
    B,
}

impl Id {
    pub fn byte(self) -> u8 {
        match self { Self::A => b'A', Self::B => b'B' }
    }

    pub fn width(self) -> usize {
        match self { Self::A => 160, Self::B => 176 }
    }
}

pub const HEIGHT: usize = 120;
pub const TITLE_HEIGHT: usize = 24;
pub const BORDER: usize = 2;

pub fn send_all(fd: i32, mut bytes: &[u8]) -> Result<(), i32> {
    while !bytes.is_empty() {
        let sent = socket::send(fd, bytes, 0)?;
        if sent == 0 { return Err(PROTOCOL_ERROR); }
        bytes = &bytes[sent..];
    }
    Ok(())
}

fn read_exact<const N: usize>(fd: i32) -> Result<[u8; N], i32> {
    let mut bytes = [0; N];
    let mut used = 0;
    while used < bytes.len() {
        let received = socket::recv(fd, &mut bytes[used..], 0)?;
        if received == 0 { return Err(PROTOCOL_ERROR); }
        used += received;
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Stamp {
    pub count: u16,
    pub serial: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Message {
    pub op: u8,
    pub id: Id,
    pub stamp: Stamp,
}

impl Message {
    fn bytes(self) -> [u8; HEADER] {
        let [c0, c1] = self.stamp.count.to_le_bytes();
        let [s0, s1] = self.stamp.serial.to_le_bytes();
        [self.op, self.id.byte(), c0, c1, s0, s1]
    }

    fn parse(bytes: &[u8]) -> Result<Self, i32> {
        let id = match bytes[1] {
            b'A' => Id::A,
            b'B' => Id::B,
            _ => return Err(PROTOCOL_ERROR),
        };
        Ok(Self {
            op: bytes[0], id,
            stamp: Stamp {
                count: u16::from_le_bytes([bytes[2], bytes[3]]),
                serial: u16::from_le_bytes([bytes[4], bytes[5]]),
            },
        })
    }

    pub fn send(self, fd: i32) -> Result<(), i32> { send_all(fd, &self.bytes()) }

    pub fn receive(fd: i32) -> Result<Self, i32> {
        Self::parse(&read_exact::<HEADER>(fd)?)
    }
}

pub struct Drawing {
    pub id: Id,
    pub stamp: Stamp,
    pub body: u32,
    pub ink: u32,
    pub x: u8,
    pub y: u8,
    pub mask: [u16; MASK_HEIGHT],
}

impl Drawing {
    pub fn send(&self, fd: i32) -> Result<(), i32> {
        let mut bytes = [0; DRAWING_BYTES];
        let header = Message { op: b'd', id: self.id, stamp: self.stamp };
        bytes[..HEADER].copy_from_slice(&header.bytes());
        bytes[6..9].copy_from_slice(&self.body.to_be_bytes()[1..]);
        bytes[9..12].copy_from_slice(&self.ink.to_be_bytes()[1..]);
        bytes[12] = self.x;
        bytes[13] = self.y;
        for (row, word) in bytes[14..].chunks_exact_mut(2).zip(self.mask) {
            row.copy_from_slice(&word.to_be_bytes());
        }
        send_all(fd, &bytes)
    }

    pub fn receive(fd: i32) -> Result<Self, i32> {
        let bytes = read_exact::<DRAWING_BYTES>(fd)?;
        let header = Message::parse(&bytes[..HEADER])?;
        let x = usize::from(bytes[12]);
        let y = usize::from(bytes[13]);
        if header.op != b'd' || x < BORDER || y < TITLE_HEIGHT
            || x + MASK_WIDTH * SCALE > header.id.width() - BORDER
            || y + MASK_HEIGHT * SCALE > HEIGHT - BORDER {
            return Err(PROTOCOL_ERROR);
        }
        Ok(Self {
            id: header.id,
            stamp: header.stamp,
            body: u32::from_be_bytes([0, bytes[6], bytes[7], bytes[8]]),
            ink: u32::from_be_bytes([0, bytes[9], bytes[10], bytes[11]]),
            x: bytes[12], y: bytes[13],
            mask: core::array::from_fn(|i| u16::from_be_bytes([bytes[14 + 2*i], bytes[15 + 2*i]])),
        })
    }
}
