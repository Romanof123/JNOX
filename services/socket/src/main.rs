#![no_std]
#![no_main]

use core::{arch::asm, panic::PanicInfo};
use jnox_native::{exit, ipc};
mod local;
use local::Local;

const CLIENTS: usize = 3;
const PATH: usize = 31;
const PAYLOAD: usize = 64;
const BUFFER: usize = PATH + PAYLOAD;

#[derive(Clone, Copy)]
struct Request {
    op: u8, fd: u8, arg: u8, path: usize, payload: usize, nonblock: bool,
    bytes: [u8; BUFFER], used: usize,
}

impl Request {
    fn begin(w: u64) -> Result<Self, i32> {
        let [reserved, flags, payload, path, arg, fd, op, _] = w.to_le_bytes();
        let valid = match op {
            1 => fd == 0 && path == 0 && payload == 0,
            2 | 3 => path > 0 && arg == 0 && payload == 0,
            4 | 8 => path == 0 && payload == 0,
            5 | 9 => path == 0 && payload == 0 && arg == 0,
            6 => arg == 0,
            7 => usize::from(arg) <= PAYLOAD && path == 0 && payload == 0,
            _ => false,
        };
        if !valid || reserved != 0 || flags > 1 || usize::from(path) > PATH ||
            usize::from(payload) > PAYLOAD { return Err(-22); }
        Ok(Self { op, fd, arg, path: path.into(), payload: payload.into(), nonblock: flags != 0,
            bytes: [0; BUFFER], used: 0 })
    }

    fn chunk(&mut self, w: u64) -> Result<u64, i32> {
        let data = w.to_le_bytes();
        let left = self.path + self.payload - self.used;
        let n = left.min(6);
        if n == 0 || usize::from(data[6]) != self.used || data[n..6].iter().any(|&v| v != 0) {
            return Err(-22);
        }
        self.bytes[self.used..self.used + n].copy_from_slice(&data[..n]);
        self.used += n;
        Ok(0)
    }
}

#[derive(Clone, Copy)]
struct Response {
    bytes: [u8; BUFFER],
    count: usize, full: usize, path: usize,
}
impl Response {
    fn read(&self, offset: usize) -> Result<u64, i32> {
        let len = self.count + self.path;
        if offset >= len { return Err(-22); }
        let n = (len - offset).min(6);
        let mut data = [0; 8];
        data[..n].copy_from_slice(&self.bytes[offset..offset+n]);
        Ok(u64::from_le_bytes(data))
    }
}

#[derive(Clone, Copy)]
struct Client {
    request: Option<Request>, response: Option<Response>, pending: Option<u64>,
}
const EMPTY: Client = Client { request: None, response: None, pending: None };

// app.ld places .data.* in the process-private RW load. Initialize the live
// socket table in its final location, without constructing it on the EL0 stack.
#[link_section = ".data.socket_service"]
static mut SERVICE: Local = Local::new();
static mut CLIENT_STATE: [Client; CLIENTS] = [EMPTY; CLIENTS];

fn execute(s: &mut Local, id: usize, r: &Request) -> Result<(u64, Option<Response>), i32> {
    let fd = r.fd;
    let path = &r.bytes[..r.path];
    let count = match r.op {
        1 => usize::from(s.socket(id, r.arg)?),
        2 => { s.bind(id, fd, path)?; 0 }
        3 => { s.connect(id, fd, path)?; 0 }
        4 => { s.listen(id, fd, r.arg.into())?; 0 }
        5 => usize::from(s.accept(id, fd)?),
        6 => s.send(id, fd, &r.bytes[r.path..r.used], (!path.is_empty()).then_some(path))?,
        7 => {
            let mut response = Response { bytes: [0; BUFFER], count: 0, full: 0, path: 0 };
            let got = s.recv(id, fd, &mut response.bytes[..usize::from(r.arg)])?;
            response.count = got.count;
            response.full = got.full_len;
            response.path = got.source_len;
            response.bytes[got.count..got.count + got.source_len].copy_from_slice(&got.source[..got.source_len]);
            return Ok((got.count as u64, Some(response)));
        }
        8 => { s.shutdown(id, fd, r.arg)?; 0 }
        9 => { s.close(id, fd)?; 0 }
        _ => return Err(-22),
    };
    Ok((count as u64, None))
}

// None transfers the reply token to this client's bounded pending slot.
fn commit(s: &mut Local, c: &mut Client, id: usize, token: u64) -> Option<Result<u64, i32>> {
    let Some(r) = c.request.as_ref() else { return Some(Err(-22)) };
    if r.used != r.path + r.payload { return Some(Err(-22)); }
    let result = execute(s, id, r);
    if matches!(result, Err(-11)) && !r.nonblock && matches!(r.op, 3 | 5 | 6 | 7) {
        c.pending = Some(token);
        return None;
    }
    c.pending = None;
    c.request = None;
    Some(result.map(|(n, response)| { c.response = response; n }))
}

fn request(s: &mut Local, c: &mut Client, id: usize, token: u64, w: u64) -> Option<Result<u64, i32>> {
    // The kernel permits only one outstanding CALL per authenticated slot.
    // A new token proves the older, still-pending call was canceled.
    if c.pending.is_some() { *c = EMPTY; }
    let tag = (w >> 56) as u8;
    let args = w & 0x00ff_ffff_ffff_ffff;
    Some(match tag {
        0x11 if c.request.is_none() => Request::begin(w).map(|r| {
            c.request = Some(r); c.response = None; 0
        }),
        0x11 => Err(-16),
        0x12 => c.request.as_mut().ok_or(-22).and_then(|r| r.chunk(w)),
        0x13 if args == 0 => return commit(s, c, id, token),
        0x14 if args == 0 => c.response.as_ref().ok_or(-22).map(|r|
            (r.count | (r.full << 8) | (r.path << 16)) as u64),
        0x15 if args & 0x0000_ffff_ffff_ffff == 0 => c.response.as_ref().ok_or(-22)
            .and_then(|r| r.read((args >> 48) as usize)),
        0x16 if args == 0 => { *c = EMPTY; Ok(0) }
        _ => Err(-22),
    })
}

fn answer(token: u64, result: Result<u64, i32>) -> Result<(), i64> {
    let word = result.unwrap_or_else(|errno| i64::from(errno) as u64);
    match ipc::reply(token, word) {
        Err(-116) => Ok(()), // The kernel already canceled this one-shot reply.
        other => other,
    }
}

fn retry(s: &mut Local, clients: &mut [Client; CLIENTS]) -> Result<(), i64> {
    // Each successful pass removes at least one token. No new requests arrive
    // during the scan and each owner can hold at most one pending transaction.
    loop {
        let mut completed = false;
        for (id, c) in clients.iter_mut().enumerate() {
            if let Some(token) = c.pending {
                if let Some(reply) = commit(s, c, id, token) {
                    answer(token, reply)?;
                    completed = true;
                }
            }
        }
        if !completed { return Ok(()); }
    }
}

fn receive() -> Result<(u64, u64, usize), i64> {
    let (status, token, word, sender): (i64,u64,u64,usize);
    unsafe {
        asm!("svc #0x4a01", in("x8") 37u64, lateout("x0") status, lateout("x1") token,
             lateout("x2") word, lateout("x3") sender, options(nostack));
    }
    if status == 0 { Ok((token,word,sender)) } else { Err(status) }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 2 || argv.is_null() { return 81; }
    let arg = *argv.add(1);
    if arg.is_null() || !(b'1'..=b'7').contains(&*arg) || *arg.add(1) != 0 { return 81; }
    let mut live = *arg - b'0';
    // Native startup calls main once. Only this single-threaded broker loop
    // accesses these objects, including while a kernel receive is blocked.
    let service = &mut *core::ptr::addr_of_mut!(SERVICE);
    let clients = &mut *core::ptr::addr_of_mut!(CLIENT_STATE);
    while live != 0 {
        let Ok((token, word, id)) = receive() else { return 82 };
        if id >= CLIENTS || live & (1 << id) == 0 { return 83; }
        if token == 0 {
            service.exit(id);
            clients[id] = EMPTY;
            live &= !(1 << id);
        } else if let Some(result) = request(service, &mut clients[id], id, token, word) {
            if answer(token, result).is_err() { return 84; }
        }
        if retry(service, clients).is_err() { return 84; }
    }
    0
}
