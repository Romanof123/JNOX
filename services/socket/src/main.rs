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
const SEND_RIGHTS: u8 = 15;
const RECV_RIGHTS: u8 = 16;
const GETSOCKNAME: u8 = 17;
const GETPEERNAME: u8 = 18;

#[derive(Clone, Copy)]
struct Request {
    op: u8, fd: u8, arg: u8, path: usize, payload: usize, nonblock: bool,
    accept_right: bool, cloexec: bool,
    bytes: [u8; BUFFER], used: usize,
}

impl Request {
    fn begin(w: u64) -> Result<Self, i32> {
        let [reserved, flags, payload, path, arg, fd, op, _] = w.to_le_bytes();
        let valid = match op {
            1 => fd == 0 && path == 0 && payload == 0,
            2 | 3 => path > 0 && arg == 0 && payload == 0,
            4 | 8 => path == 0 && payload == 0,
            5 | 9 | GETSOCKNAME | GETPEERNAME => path == 0 && payload == 0 && arg == 0,
            6 => arg == 0,
            7 | RECV_RIGHTS => usize::from(arg) <= PAYLOAD && path == 0 && payload == 0,
            SEND_RIGHTS => path == 0 && payload != 0,
            _ => false,
        };
        let allowed_flags = if op == RECV_RIGHTS { 7 } else { 1 };
        if !valid || reserved != 0 || flags > allowed_flags || usize::from(path) > PATH ||
            usize::from(payload) > PAYLOAD { return Err(-22); }
        Ok(Self { op, fd, arg, path: path.into(), payload: payload.into(),
            nonblock: flags & 1 != 0, accept_right: flags & 2 != 0, cloexec: flags & 4 != 0,
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
    received_fd: u8, truncated: bool,
}
impl Response {
    fn meta(&self) -> u64 {
        (self.count | self.full << 8 | self.path << 16) as u64
            | u64::from(self.received_fd) << 24 | u64::from(self.truncated) << 32
    }
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
    request: Option<Request>, response: Option<Response>, pending: Option<u64>, poll: Option<u64>,
}
const EMPTY: Client = Client { request: None, response: None, pending: None, poll: None };

// app.ld places .data.* in the process-private RW load. Initialize the live
// socket table in its final location, without constructing it on the EL0 stack.
#[link_section = ".data.socket_service"]
static mut BANKS: [Local; 2] = [Local::new(), Local::new()];
static mut CLIENT_STATE: [Client; CLIENTS] = [EMPTY; CLIENTS];

#[derive(Clone, Copy)]
enum Transfer {
    None,
    Hold(u8),
    Install(u64, bool),
}

#[derive(Clone, Copy)]
struct Reply { word: u64, transfer: Transfer }

impl Reply {
    fn plain(word: u64) -> Self { Self { word, transfer: Transfer::None } }

    // A successful special reply transfers ownership and accepts the original
    // COMMIT at the same kernel boundary. On failure the ordinary error reply
    // may itself see ESTALE if the client deadline has expired.
    fn deliver(self, token: u64, client: &mut Client) -> Result<bool, i64> {
        let (op, argument, word) = match self.transfer {
            Transfer::None => {
                ipc::reply(token, self.word)?;
                return Ok(true);
            }
            Transfer::Hold(fd) => (184, u64::from(fd), self.word),
            Transfer::Install(escrow, cloexec) => (185, escrow, self.word | (u64::from(cloexec) << 8)),
        };
        match rights_call(op, token, word, argument) {
            Ok(value) => {
                match self.transfer {
                    Transfer::Hold(_) if value != token => return Err(-5),
                    Transfer::Install(_, _) => {
                        if !(96..=103).contains(&value) { return Err(-5); }
                        client.response.as_mut().ok_or(-5i64)?.received_fd = value as u8;
                    }
                    _ => {}
                }
                Ok(true)
            }
            Err(errno) => {
                client.response = None;
                ipc::reply(token, errno as u64)?;
                Ok(false)
            }
        }
    }
}

fn rights_call(op: u64, argument: u64, word: u64, right: u64) -> Result<u64, i64> {
    let status: i64;
    let value: u64;
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") argument => status,
             inout("x1") word => value, inout("x2") right => _, options(nostack));
    }
    if status == 0 { Ok(value) } else { Err(status) }
}

fn execute(s: &mut Local, id: usize, r: &Request, token: u64)
    -> Result<(Reply, Option<Response>), i32>
{
    let fd = r.fd;
    let path = &r.bytes[..r.path];
    let count = match r.op {
        1 => usize::from(s.socket(id, r.arg)?),
        2 => { s.bind(id, fd, path)?; 0 }
        3 => { s.connect(id, fd, path)?; 0 }
        4 => { s.listen(id, fd, r.arg.into())?; 0 }
        5 => usize::from(s.accept(id, fd)?),
        6 => s.send(id, fd, &r.bytes[r.path..r.used], (!path.is_empty()).then_some(path))?,
        SEND_RIGHTS => {
            if !(96..=103).contains(&r.arg) { return Err(-9); }
            let n = s.send_rights(id, fd, &r.bytes[..r.used], token)?;
            return Ok((Reply { word: n as u64, transfer: Transfer::Hold(r.arg) }, None));
        }
        7 | RECV_RIGHTS => {
            if r.op == RECV_RIGHTS { s.require_stream(id, fd)?; }
            let mut response = Response { bytes: [0; BUFFER], count: 0, full: 0, path: 0,
                received_fd: 0, truncated: false };
            let got = s.recv(id, fd, &mut response.bytes[..usize::from(r.arg)])?;
            response.count = got.count;
            response.full = got.full_len;
            response.path = got.source_len;
            response.bytes[got.count..got.count + got.source_len].copy_from_slice(&got.source[..got.source_len]);
            let transfer = if got.escrow != 0 && r.accept_right {
                Transfer::Install(got.escrow, r.cloexec)
            } else {
                response.truncated = r.op == RECV_RIGHTS && got.escrow != 0;
                Transfer::None
            };
            return Ok((Reply { word: got.count as u64, transfer }, Some(response)));
        }
        8 => { s.shutdown(id, fd, r.arg)?; 0 }
        9 => { s.close(id, fd)?; 0 }
        GETSOCKNAME | GETPEERNAME => {
            let name = s.address(id, fd, r.op == GETPEERNAME)?;
            let mut response = Response { bytes: [0; BUFFER], count: 0, full: 0, path: name.len(),
                received_fd: 0, truncated: false };
            response.bytes[..name.len()].copy_from_slice(name);
            return Ok((Reply::plain(0), Some(response)));
        }
        _ => return Err(-22),
    };
    Ok((Reply::plain(count as u64), None))
}

// None transfers the reply token to this client's bounded pending slot.
fn commit(s: &mut Local, c: &mut Client, id: usize, token: u64) -> Option<Result<Reply, i32>> {
    let Some(r) = c.request.as_ref() else { return Some(Err(-22)) };
    if r.used != r.path + r.payload { return Some(Err(-22)); }
    let result = execute(s, id, r, token);
    if matches!(result, Err(-11)) && !r.nonblock && matches!(r.op, 3 | 5 | 6 | 7 | SEND_RIGHTS | RECV_RIGHTS) {
        c.pending = Some(token);
        return None;
    }
    c.pending = None;
    c.request = None;
    Some(result.map(|(n, response)| { c.response = response; n }))
}

fn request(s: &mut Local, c: &mut Client, id: usize, token: u64, w: u64) -> Option<Result<Reply, i32>> {
    // The kernel permits only one outstanding CALL per authenticated slot.
    // A new token proves the older, still-pending call was canceled.
    debug_assert!(c.pending.is_none());
    let tag = (w >> 56) as u8;
    let args = w & 0x00ff_ffff_ffff_ffff;
    Some(match tag {
        0x11 if c.request.is_none() => Request::begin(w).map(|r| {
            c.request = Some(r); c.response = None; 0
        }),
        0x11 => Err(-16),
        0x12 => c.request.as_mut().ok_or(-22).and_then(|r| r.chunk(w)),
        0x13 if args == 0 => return commit(s, c, id, token),
        0x14 if args == 0 => c.response.as_ref().ok_or(-22).map(Response::meta),
        0x15 if args & 0x0000_ffff_ffff_ffff == 0 => c.response.as_ref().ok_or(-22)
            .and_then(|r| r.read((args >> 48) as usize)),
        0x16 if args == 0 => { *c = EMPTY; Ok(0) }
        _ => Err(-22),
    }.map(Reply::plain))
}

// A successful kernel reply consumes this unique token. The single broker
// cannot observe another request or death between that SVC and bank selection.
struct Service<'a> {
    banks: &'a mut [Local; 2],
    live: usize,
}

// Entries keep their table indices while queued. INSTALL already consumed
// its own entry. Closing a listener can discard several pending endpoints.
fn discard_removed(before: [u64; local::RIGHTS], after: [u64; local::RIGHTS], installed: u64)
    -> Result<(), i64>
{
    let mut status = Ok(());
    for (old, new) in before.into_iter().zip(after) {
        if old != 0 && old != new && old != installed {
            if let Err(errno) = rights_call(186, old, 0, 0) { status = Err(errno); }
        }
    }
    status
}

impl Service<'_> {
    fn trial_bank(&mut self) -> usize {
        let (left, right) = self.banks.split_at_mut(1);
        if self.live == 0 { right[0].copy_from(&left[0]); }
        else { left[0].copy_from(&right[0]); }
        1 - self.live
    }

    fn publish(&mut self, bank: usize, transfer: Transfer) -> Result<(), i64> {
        let previous = self.banks[self.live].escrows();
        self.live = bank;
        let installed = match transfer { Transfer::Install(escrow, _) => escrow, _ => 0 };
        discard_removed(previous, self.banks[bank].escrows(), installed)
    }

    fn step(&mut self, client: &mut Client, id: usize, token: u64, word: Option<u64>)
        -> Result<bool, i64>
    {
        if word.is_some() && client.pending.is_some() {
            // Receiving another authenticated call from this PID proves that
            // its former one-shot call ended, even if the new call also expires.
            *client = EMPTY;
        }
        // POLL uses one authenticated call. A canceled wait is displaced above
        // by the next call from the same owner. No socket state is mutated.
        let poll = match word {
            Some(w) if w >> 56 == 0x17 => Some(w),
            None => client.poll,
            _ => None,
        };
        if let Some(w) = poll {
            let mut ready = 0u64;
            let mut valid = client.request.is_none() && w & 0x00ff_fffe_0000_0000 == 0;
            for fd in 0..4 {
                let interest = (w >> (fd * 8)) as u8;
                if interest == 0 { continue; }
                valid &= interest & !0x85 == 0 && interest & 0x80 != 0;
                ready |= u64::from(self.banks[self.live].poll_slot(id, fd as u8, interest & 5)) << (fd * 8);
            }
            if valid && ready == 0 && w & (1u64 << 32) == 0 {
                client.pending = Some(token);
                client.poll = Some(w);
                return Ok(false);
            }
            client.pending = None;
            client.poll = None;
            match ipc::reply(token, if valid { ready } else { (-22i64) as u64 }) {
                Ok(()) | Err(-116) => return Ok(true),
                Err(e) => return Err(e),
            }
        }
        let mut trial = *client;
        let is_commit = word.is_none() || word == Some(0x13u64 << 56);
        let bank = if is_commit {
            self.trial_bank()
        } else {
            // Non-COMMIT framing operations only access the private client copy.
            self.live
        };
        let result = match word {
            Some(w) => request(&mut self.banks[bank], &mut trial, id, token, w),
            None => commit(&mut self.banks[bank], &mut trial, id, token),
        };
        let Some(result) = result else {
            client.pending = Some(token);
            return Ok(false);
        };
        let reply = result.unwrap_or_else(|errno| Reply::plain(i64::from(errno) as u64));
        match reply.deliver(token, &mut trial) {
            Ok(accepted) => {
                if is_commit && result.is_ok() && accepted { self.publish(bank, reply.transfer)?; }
                *client = trial;
            }
            Err(-116) => {
                // A canceled COMMIT ends this request without publishing a
                // descriptor, bytes, response metadata, or a retryable token.
                if is_commit { *client = EMPTY; }
            }
            Err(errno) => return Err(errno),
        }
        Ok(true)
    }

    fn retry(&mut self, clients: &mut [Client; CLIENTS]) -> Result<(), i64> {
        loop {
            let mut completed = false;
            for (id, client) in clients.iter_mut().enumerate() {
                if let Some(token) = client.pending {
                    completed |= self.step(client, id, token, None)?;
                }
            }
            if !completed { return Ok(()); }
        }
    }

    fn serve(&mut self, clients: &mut [Client; CLIENTS], mut live: u8) -> i32 {
        while live != 0 {
            let Ok((token, word, id)) = receive() else { return 82 };
            if id >= CLIENTS || live & (1 << id) == 0 { return 83; }
            if token == 0 {
                let bank = self.trial_bank();
                self.banks[bank].exit(id);
                if self.publish(bank, Transfer::None).is_err() { return 85; }
                clients[id] = EMPTY;
                live &= !(1 << id);
            } else if self.step(&mut clients[id], id, token, Some(word)).is_err() {
                return 84;
            }
            if self.retry(clients).is_err() { return 84; }
        }
        0
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
    let live = *arg - b'0';
    // Native startup calls main once. Only this single-threaded broker loop
    // accesses these objects, including while a kernel receive is blocked.
    let mut service = Service { banks: &mut *core::ptr::addr_of_mut!(BANKS), live: 0 };
    let clients = &mut *core::ptr::addr_of_mut!(CLIENT_STATE);
    let status = service.serve(clients, live);
    // Also release live queues on a controlled error exit. EL1 reclaims the
    // broker's entire escrow table if execution terminates before this point.
    let remaining = service.banks[service.live].escrows();
    if discard_removed(remaining, [0; local::RIGHTS], 0).is_err() { return 85; }
    status
}
