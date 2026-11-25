#![no_std]
#![no_main]

use core::{arch::asm, mem, panic::PanicInfo, ptr};
use jnox_native::{exit, ipc};
mod state;
use state::{Handle, PubSub, Record, CLIENTS, PAYLOAD, TOPICS};

const SUBSCRIBE: u8 = 1;
const UNSUBSCRIBE: u8 = 2;
const PUBLISH: u8 = 3;
const OBSERVE: u8 = 4;
const RECEIVE: u8 = 5;
const PEEK: u8 = 6;
const MAX_RESPONSE: usize = 9 + PAYLOAD;
type ResultCode<T> = Result<T, i32>;

#[derive(Clone, Copy)]
struct Request {
    op: u8,
    topic: usize,
    wait: bool,
    size: usize,
    used: usize,
    bytes: [u8; PAYLOAD],
}
impl Request {
    fn begin(word: u64) -> ResultCode<Self> {
        let [size, topic, op, flags, a, b, c, _] = word.to_le_bytes();
        if a | b | c != 0 || (flags != 0 && !(flags == 1 && op == RECEIVE)) {
            return Err(-22);
        }
        let size = usize::from(size);
        let topic = usize::from(topic);
        let valid = match op {
            SUBSCRIBE => topic < TOPICS && size == 0,
            PUBLISH => {
                if size > PAYLOAD {
                    return Err(-90);
                }
                topic < TOPICS
            }
            UNSUBSCRIBE | OBSERVE | RECEIVE | PEEK => topic == 0 && size == 16,
            _ => false,
        };
        if !valid {
            return Err(-22);
        }
        Ok(Self {
            op,
            topic,
            wait: flags == 1,
            size,
            used: 0,
            bytes: [0; PAYLOAD],
        })
    }
    fn chunk(&mut self, word: u64) -> ResultCode<()> {
        let bytes = word.to_le_bytes();
        let n = (self.size - self.used).min(6);
        if n == 0 || usize::from(bytes[6]) != self.used || bytes[n..6].iter().any(|&v| v != 0) {
            return Err(-22);
        }
        self.bytes[self.used..self.used + n].copy_from_slice(&bytes[..n]);
        self.used += n;
        Ok(())
    }
    fn handle(&self) -> ResultCode<Handle> {
        let slot = u64::from_le_bytes(self.bytes[0..8].try_into().unwrap());
        let generation = u64::from_le_bytes(self.bytes[8..16].try_into().unwrap());
        Ok(Handle {
            slot: u8::try_from(slot).map_err(|_| -22)?,
            generation,
        })
    }
}

#[derive(Clone, Copy)]
struct Response {
    bytes: [u8; MAX_RESPONSE],
    len: usize,
}
impl Response {
    const EMPTY: Self = Self {
        bytes: [0; MAX_RESPONSE],
        len: 0,
    };
    fn append(&mut self, bytes: &[u8]) {
        let end = self.len + bytes.len();
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;
    }
    fn record(&mut self, r: Record) {
        self.append(&r.sequence.to_le_bytes());
        self.append(&[r.len]);
        self.append(&r.data[..usize::from(r.len)]);
    }
    fn read(&self, offset: usize) -> ResultCode<u64> {
        if offset >= self.len {
            return Err(-22);
        }
        let n = (self.len - offset).min(6);
        let mut chunk = [0; 8];
        chunk[..n].copy_from_slice(&self.bytes[offset..offset + n]);
        Ok(u64::from_le_bytes(chunk))
    }
}

#[derive(Clone, Copy)]
struct Client {
    request: Option<Request>,
    reply: Response,
    waiting: Option<(u64, Handle)>,
}
impl Client {
    const EMPTY: Self = Self {
        request: None,
        reply: Response::EMPTY,
        waiting: None,
    };
}

fn execute(s: &mut PubSub, id: usize, r: &Request) -> ResultCode<Response> {
    let mut out = Response::EMPTY;
    match r.op {
        SUBSCRIBE => {
            let h = s.subscribe(id, r.topic)?;
            out.append(&u64::from(h.slot).to_le_bytes());
            out.append(&h.generation.to_le_bytes());
        }
        UNSUBSCRIBE => s.unsubscribe(id, r.handle()?)?,
        PUBLISH => out.append(&s.publish(id, r.topic, &r.bytes[..r.size])?.to_le_bytes()),
        OBSERVE => {
            let obs = s.observe(id, r.handle()?)?;
            out.append(&[obs.queued, obs.high_water]);
            out.append(&obs.lost.to_le_bytes());
            out.append(&obs.front_sequence.unwrap_or(0).to_le_bytes());
        }
        RECEIVE => out.record(s.receive(id, r.handle()?)?),
        PEEK => out.record(s.peek(id, r.handle()?)?),
        _ => return Err(-22),
    }
    Ok(out)
}

struct Broker<'a> {
    current: &'a mut PubSub,
    staged: &'a mut PubSub,
    clients: &'a mut [Client; CLIENTS],
    live: u8,
    stale_replies: u64,
    peak_waits: usize,
    commits: u64,
}
impl Broker<'_> {
    // A reply accepted by the kernel is the single commit point. An expired
    // token never selects its prepared state or exposes its response bytes.
    fn answer(&mut self, token: u64, value: ResultCode<u64>) -> Result<bool, i64> {
        let word = value.unwrap_or_else(|e| i64::from(e) as u64);
        match ipc::reply(token, word) {
            Ok(()) => Ok(true),
            Err(-116) => {
                self.stale_replies = self.stale_replies.saturating_add(1);
                Ok(false)
            }
            Err(other) => Err(other),
        }
    }
    fn finish(
        &mut self,
        id: usize,
        token: u64,
        result: ResultCode<Response>,
        mutates: bool,
    ) -> Result<bool, i64> {
        match result {
            Ok(out) if self.answer(token, Ok(out.len as u64))? => {
                if mutates {
                    mem::swap(&mut self.current, &mut self.staged);
                    self.commits = self.commits.saturating_add(1);
                }
                self.clients[id].reply = out;
                Ok(true)
            }
            Ok(_) => Ok(false),
            Err(code) => {
                self.answer(token, Err(code))?;
                Ok(false)
            }
        }
    }
    fn receive_or_wait(&mut self, id: usize, token: u64, h: Handle) -> Result<(), i64> {
        match self.current.peek(id, h) {
            Err(-11) => match self.current.has_live_publisher(id, h) {
                Ok(true) => {
                    self.clients[id].waiting = Some((token, h));
                    self.peak_waits = self
                        .peak_waits
                        .max(self.clients.iter().filter(|c| c.waiting.is_some()).count());
                }
                Ok(false) => {
                    self.answer(token, Err(-32))?;
                }
                Err(e) => {
                    self.answer(token, Err(e))?;
                }
            },
            Err(e) => {
                self.answer(token, Err(e))?;
            }
            Ok(_) => {
                self.staged.copy_from(self.current);
                let result = self.staged.receive(id, h).map(|record| {
                    let mut response = Response::EMPTY;
                    response.record(record);
                    response
                });
                self.finish(id, token, result, true)?;
            }
        }
        Ok(())
    }
    fn wake(&mut self) -> Result<(), i64> {
        // One pass suffices: completing a receive removes one queued sample.
        for pid in 0..CLIENTS {
            if let Some((token, h)) = self.clients[pid].waiting.take() {
                self.receive_or_wait(pid, token, h)?;
            }
        }
        Ok(())
    }
    fn commit(&mut self, id: usize, token: u64) -> Result<bool, i64> {
        let Some(req) = self.clients[id].request else {
            self.answer(token, Err(-22))?;
            return Ok(false);
        };
        if req.used != req.size {
            self.answer(token, Err(-22))?;
            return Ok(false);
        }
        self.clients[id].request = None;
        if req.op == RECEIVE && req.wait {
            match req.handle() {
                Ok(h) => self.receive_or_wait(id, token, h)?,
                Err(e) => {
                    self.answer(token, Err(e))?;
                }
            }
            return Ok(false);
        }
        let mutates = matches!(req.op, SUBSCRIBE | UNSUBSCRIBE | PUBLISH | RECEIVE);
        let result = if mutates {
            self.staged.copy_from(self.current);
            execute(self.staged, id, &req)
        } else {
            execute(self.current, id, &req)
        };
        Ok(self.finish(id, token, result, mutates)? && req.op == PUBLISH)
    }
    fn dispatch(&mut self, id: usize, token: u64, word: u64) -> Result<bool, i64> {
        // A second call from this authenticated process can arrive only after
        // the kernel has ended its previous one. No pending call is replayed.
        if self.clients[id].waiting.take().is_some() {
            self.clients[id] = Client::EMPTY;
        }
        let tag = (word >> 56) as u8;
        let args = word & 0x00ff_ffff_ffff_ffff;
        let mut next = self.clients[id];
        let result = match tag {
            0x31 if next.request.is_none() => Request::begin(word).map(|r| {
                next.request = Some(r);
                next.reply = Response::EMPTY;
                0
            }),
            0x31 => Err(-16),
            0x32 => next
                .request
                .as_mut()
                .ok_or(-22)
                .and_then(|r| r.chunk(word))
                .map(|()| 0),
            0x33 if args == 0 => return self.commit(id, token),
            0x34 if args & 0x0000_ffff_ffff_ffff == 0 => next.reply.read((args >> 48) as usize),
            0x35 if args == 0 => {
                next = Client::EMPTY;
                Ok(0)
            }
            _ => Err(-22),
        };
        if self.answer(token, result)? && result.is_ok() {
            self.clients[id] = next
        }
        Ok(false)
    }
    fn exit_client(&mut self, id: usize) {
        self.clients[id] = Client::EMPTY;
        self.current.exit(id);
        self.live &= !(1 << id);
    }
    fn report(&self) {
        let s = self.current.stats();
        let waiting = self.clients.iter().filter(|c| c.waiting.is_some()).count();
        let rows = [
            MAGIC,
            u64::from(self.live),
            s.active as u64,
            s.queued as u64,
            s.peak_active as u64,
            s.peak_queued as u64,
            s.dropped,
            s.last_sequence[0],
            s.last_sequence[1],
            s.last_sequence[2],
            s.last_sequence[3],
            self.stale_replies,
            waiting as u64,
            self.peak_waits as u64,
            self.commits,
        ];
        unsafe { ptr::write_volatile(&raw mut pubsub_report, rows) }
    }
}

fn receive_event() -> Result<(u64, u64, usize), i64> {
    let (status, token, word, id): (i64, u64, u64, usize);
    unsafe {
        asm!("svc #0x4a01",in("x8") 52u64,lateout("x0")status,lateout("x1")token,
             lateout("x2")word,lateout("x3")id,options(nostack));
    }
    if status == 0 {
        Ok((token, word, id))
    } else {
        Err(status)
    }
}

const MAGIC: u64 = 0x4a4e505338;
#[used]
#[no_mangle]
#[link_section = ".data.private_report"]
static mut pubsub_report: [u64; 15] = [MAGIC, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
static mut LIVE: PubSub = PubSub::unconfigured();
static mut STAGED: PubSub = PubSub::unconfigured();
static mut CLIENT_STATE: [Client; CLIENTS] = [Client::EMPTY; CLIENTS];

/// The loader constructs both argv strings in the new process's private image.
unsafe fn grant_arg(arg: *const u8) -> Option<[u8; 4]> {
    if arg.is_null() {
        return None;
    }
    let mut masks = [0u8; 4];
    for (pos, mask) in masks[..CLIENTS].iter_mut().enumerate() {
        *mask = match *arg.add(pos) {
            b @ b'0'..=b'9' => b - b'0',
            b @ b'a'..=b'f' => 10 + b - b'a',
            _ => return None,
        };
    }
    (*arg.add(CLIENTS) == 0).then_some(masks)
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 3 || argv.is_null() {
        return 81;
    }
    let (Some(publish), Some(subscribe)) = (grant_arg(*argv.add(1)), grant_arg(*argv.add(2)))
    else {
        return 81;
    };
    let mut broker = Broker {
        current: &mut *(&raw mut LIVE),
        staged: &mut *(&raw mut STAGED),
        clients: &mut *(&raw mut CLIENT_STATE),
        live: 0b111,
        stale_replies: 0,
        peak_waits: 0,
        commits: 0,
    };
    if broker.current.configure_grants(publish, subscribe).is_err() {
        return 81;
    }
    broker.report();
    while broker.live != 0 {
        let Ok((token, word, id)) = receive_event() else {
            return 82;
        };
        if id >= CLIENTS || broker.live & (1 << id) == 0 {
            return 83;
        }
        let wake = if token == 0 {
            broker.exit_client(id);
            true
        } else {
            match broker.dispatch(id, token, word) {
                Ok(w) => w,
                Err(_) => return 84,
            }
        };
        if wake && broker.wake().is_err() {
            return 84;
        }
        broker.report();
    }
    0
}
