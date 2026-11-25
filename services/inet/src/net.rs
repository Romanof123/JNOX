use crate::device::Frames;
use smoltcp::{
    iface::{Config, Interface, PollIngressSingleResult, SocketHandle, SocketSet, SocketStorage},
    socket::{tcp, udp},
    time::{Duration, Instant},
    wire::{EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address},
};

const TCP: usize = 6;
const UDP: usize = 4;
const CLIENTS: usize = 3;
const FDS: usize = 4;
const LOCAL: Ipv4Address = Ipv4Address::new(10, 0, 2, 15);
const GATEWAY: Ipv4Address = Ipv4Address::new(10, 0, 2, 2);
const UDP_MAX: usize = 32;

struct TcpBuffers {
    rx: [u8; 1024],
    tx: [u8; 128],
}
struct UdpBuffers {
    rx: [u8; 128],
    tx: [u8; 128],
    rxmeta: [udp::PacketMetadata; 4],
    txmeta: [udp::PacketMetadata; 4],
}
pub struct Storage {
    sockets: [SocketStorage<'static>; TCP + UDP],
    tcp: [TcpBuffers; TCP],
    udp: [UdpBuffers; UDP],
}
impl Storage {
    pub const EMPTY: Self = Self {
        sockets: [const { SocketStorage::EMPTY }; TCP + UDP],
        tcp: [const {
            TcpBuffers {
                rx: [0; 1024],
                tx: [0; 128],
            }
        }; TCP],
        udp: [const {
            UdpBuffers {
                rx: [0; 128],
                tx: [0; 128],
                rxmeta: [udp::PacketMetadata::EMPTY; 4],
                txmeta: [udp::PacketMetadata::EMPTY; 4],
            }
        }; UDP],
    };
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Connecting(IpEndpoint, Instant),
    Connected,
}
#[derive(Clone, Copy)]
enum Kind {
    Stream(usize, Phase),
    Datagram(usize, Option<IpEndpoint>),
    Listen {
        pending: [Option<usize>; 2],
        limit: usize,
    },
}
#[derive(Clone, Copy)]
struct Descriptor {
    kind: Kind,
    port: u16,
    rd: bool,
    wr: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Free,
    Live,
    Closing,
}
pub struct Net {
    iface: Interface,
    sockets: SocketSet<'static>,
    tcp: [SocketHandle; TCP],
    udp: [SocketHandle; UDP],
    tcp_usage: [Use; TCP],
    udp_usage: [Use; UDP],
    udp_pending: [u8; UDP],
    retire_at: [Instant; UDP],
    tcp_retire_at: [Instant; TCP],
    ready_order: [u64; TCP],
    next_order: u64,
    now: Instant,
    fin: [bool; TCP],
    fds: [[Option<Descriptor>; FDS]; CLIENTS],
    next_port: u16,
}
pub struct Received {
    pub count: usize,
    pub full: usize,
    pub peer: Option<IpEndpoint>,
}

pub fn endpoint(bytes: &[u8]) -> Result<IpEndpoint, i32> {
    let b = <&[u8; 6]>::try_from(bytes).map_err(|_| -22)?;
    Ok(IpEndpoint::new(
        IpAddress::v4(b[2], b[3], b[4], b[5]),
        u16::from_be_bytes([b[0], b[1]]),
    ))
}
pub fn encode(e: IpEndpoint) -> [u8; 6] {
    let [p0, p1] = e.port.to_be_bytes();
    let IpAddress::Ipv4(ip) = e.addr;
    let [a, b, c, d] = ip.octets();
    [p0, p1, a, b, c, d]
}
fn remote_ok(e: IpEndpoint) -> Result<(), i32> {
    let IpAddress::Ipv4(ip) = e.addr;
    if e.port == 0 || ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
        Err(-99)
    } else {
        Ok(())
    }
}
impl Net {
    pub fn new(store: &'static mut Storage, dev: &mut Frames, now: Instant) -> Self {
        let mut config = Config::new(EthernetAddress([0x52, 0x54, 0, 0x12, 0x34, 0x56]).into());
        config.random_seed = 0x6a6e6f785f697034 ^ now.total_micros() as u64;
        let mut iface = Interface::new(config, dev, now);
        iface.update_ip_addrs(|a| a.push(IpCidr::new(LOCAL.into(), 24)).unwrap());
        iface.routes_mut().add_default_ipv4_route(GATEWAY).unwrap();
        let mut sockets = SocketSet::new(&mut store.sockets[..]);
        let mut tcp_handles = [SocketHandle::default(); TCP];
        let mut udp_handles = [SocketHandle::default(); UDP];
        for (h, b) in tcp_handles.iter_mut().zip(store.tcp.iter_mut()) {
            let mut s = tcp::Socket::new(
                tcp::SocketBuffer::new(&mut b.rx[..]),
                tcp::SocketBuffer::new(&mut b.tx[..]),
            );
            s.set_ack_delay(None);
            *h = sockets.add(s);
        }
        for (h, b) in udp_handles.iter_mut().zip(store.udp.iter_mut()) {
            *h = sockets.add(udp::Socket::new(
                udp::PacketBuffer::new(&mut b.rxmeta[..], &mut b.rx[..]),
                udp::PacketBuffer::new(&mut b.txmeta[..], &mut b.tx[..]),
            ));
        }
        Self {
            iface,
            sockets,
            tcp: tcp_handles,
            udp: udp_handles,
            tcp_usage: [Use::Free; TCP],
            udp_usage: [Use::Free; UDP],
            udp_pending: [0; UDP],
            retire_at: [now; UDP],
            tcp_retire_at: [now; TCP],
            ready_order: [0; TCP],
            next_order: 1,
            now,
            fin: [false; TCP],
            fds: [[None; FDS]; CLIENTS],
            next_port: 49152,
        }
    }
    fn tcp_socket(&mut self, i: usize) -> &mut tcp::Socket<'static> {
        self.sockets.get_mut(self.tcp[i])
    }
    fn udp_socket(&mut self, i: usize) -> &mut udp::Socket<'static> {
        self.sockets.get_mut(self.udp[i])
    }
    fn tcp_slot(&mut self) -> Result<usize, i32> {
        let i = self
            .tcp_usage
            .iter()
            .position(|u| *u == Use::Free)
            .ok_or(-23)?;
        self.tcp_usage[i] = Use::Live;
        self.fin[i] = false;
        self.ready_order[i] = 0;
        Ok(i)
    }
    fn fd(&self, id: usize, n: u8) -> Result<Descriptor, i32> {
        self.fds
            .get(id)
            .and_then(|fs| fs.get(usize::from(n)))
            .copied()
            .flatten()
            .ok_or(-9)
    }
    fn free_fd(&self, id: usize) -> Result<usize, i32> {
        self.fds
            .get(id)
            .ok_or(-13)?
            .iter()
            .position(Option::is_none)
            .ok_or(-24)
    }
    pub fn socket(&mut self, id: usize, kind: u8) -> Result<u8, i32> {
        let fd = self.free_fd(id)?;
        let kind = match kind {
            1 => Kind::Stream(self.tcp_slot()?, Phase::Idle),
            2 => {
                let i = self
                    .udp_usage
                    .iter()
                    .position(|&u| u == Use::Free)
                    .ok_or(-23)?;
                self.udp_usage[i] = Use::Live;
                Kind::Datagram(i, None)
            }
            _ => return Err(-94),
        };
        self.fds[id][fd] = Some(Descriptor {
            kind,
            port: 0,
            rd: true,
            wr: true,
        });
        Ok(fd as u8)
    }
    fn available(&self, port: u16, udp: bool) -> bool {
        if self
            .fds
            .iter()
            .flatten()
            .flatten()
            .any(|d| d.port == port && matches!(d.kind, Kind::Datagram(..)) == udp)
        {
            return false;
        }
        if udp {
            return !(0..UDP).any(|i| {
                self.udp_usage[i] == Use::Closing
                    && self.sockets.get::<udp::Socket>(self.udp[i]).endpoint().port == port
            });
        }
        !(0..TCP).any(|i| {
            self.tcp_usage[i] == Use::Closing
                && self
                    .sockets
                    .get::<tcp::Socket>(self.tcp[i])
                    .local_endpoint()
                    .is_some_and(|e| e.port == port)
        })
    }
    fn select_port(&mut self, wanted: u16, datagram: bool) -> Result<u16, i32> {
        if wanted != 0 {
            return if self.available(wanted, datagram) {
                Ok(wanted)
            } else {
                Err(-98)
            };
        }
        for _ in 0..32 {
            let p = self.next_port;
            self.next_port = 49152 + (p - 49152 + 1) % 32;
            if self.available(p, datagram) {
                return Ok(p);
            }
        }
        Err(-98)
    }
    pub fn bind(&mut self, id: usize, n: u8, ep: IpEndpoint) -> Result<(), i32> {
        let mut d = self.fd(id, n)?;
        if d.port != 0 {
            return Err(-22);
        }
        if ep.addr != IpAddress::Ipv4(LOCAL) && ep.addr != IpAddress::v4(0, 0, 0, 0) {
            return Err(-99);
        }
        d.port = self.select_port(ep.port, matches!(d.kind, Kind::Datagram(..)))?;
        if let Kind::Datagram(i, _) = d.kind {
            self.udp_socket(i).bind((LOCAL, d.port)).map_err(|_| -22)?;
        }
        self.fds[id][n as usize] = Some(d);
        Ok(())
    }
    fn autobind(&mut self, id: usize, n: u8) -> Result<Descriptor, i32> {
        if self.fd(id, n)?.port == 0 {
            self.bind(id, n, IpEndpoint::new(LOCAL.into(), 0))?
        }
        self.fd(id, n)
    }
    pub fn connect(&mut self, id: usize, n: u8, ep: IpEndpoint) -> Result<(), i32> {
        remote_ok(ep)?;
        let mut d = self.autobind(id, n)?;
        match d.kind {
            Kind::Datagram(i, _) => d.kind = Kind::Datagram(i, Some(ep)),
            Kind::Stream(i, Phase::Idle) => {
                let s = self.sockets.get_mut::<tcp::Socket>(self.tcp[i]);
                s.connect(self.iface.context(), ep, (LOCAL, d.port))
                    .map_err(|_| -99)?;
                self.fin[i] = false;
                d.kind = Kind::Stream(i, Phase::Connecting(ep, self.now + Duration::from_secs(15)));
                self.fds[id][n as usize] = Some(d);
                return Err(-11);
            }
            Kind::Stream(i, Phase::Connecting(target, deadline)) => {
                if target != ep {
                    return Err(-114);
                }
                match self.tcp_socket(i).state() {
                    tcp::State::Established | tcp::State::CloseWait => {
                        d.kind = Kind::Stream(i, Phase::Connected)
                    }
                    tcp::State::Closed => {
                        d.kind = Kind::Stream(i, Phase::Idle);
                        self.fds[id][n as usize] = Some(d);
                        return Err(-111);
                    }
                    _ if self.now >= deadline => {
                        self.tcp_socket(i).abort();
                        d.kind = Kind::Stream(i, Phase::Idle);
                        self.fds[id][n as usize] = Some(d);
                        return Err(-110);
                    }
                    _ => return Err(-11),
                }
            }
            _ => return Err(-106),
        }
        self.fds[id][n as usize] = Some(d);
        Ok(())
    }
    fn io_descriptor(&mut self, id: usize, n: u8) -> Result<Descriptor, i32> {
        let d = self.fd(id, n)?;
        if let Kind::Stream(_, Phase::Connecting(target, _)) = d.kind {
            self.connect(id, n, target)?;
            self.fd(id, n)
        } else {
            Ok(d)
        }
    }
    pub fn listen(&mut self, id: usize, n: u8, limit: usize) -> Result<(), i32> {
        let mut d = self.fd(id, n)?;
        if !(1..=2).contains(&limit) || d.port == 0 {
            return Err(-22);
        }
        let Kind::Stream(i, Phase::Idle) = d.kind else {
            return Err(-95);
        };
        let mut pending = [Some(i), None];
        if limit == 2 {
            pending[1] = Some(self.tcp_slot()?);
        }
        for j in pending.into_iter().flatten() {
            self.tcp_socket(j)
                .listen((LOCAL, d.port))
                .expect("reserved idle listener");
        }
        d.kind = Kind::Listen { pending, limit };
        self.fds[id][n as usize] = Some(d);
        Ok(())
    }
    pub fn accept(&mut self, id: usize, n: u8) -> Result<u8, i32> {
        let mut d = self.fd(id, n)?;
        let Kind::Listen { mut pending, limit } = d.kind else {
            return Err(-22);
        };
        let fresh = self.free_fd(id)?;
        let oldest = (0..limit)
            .filter_map(|pos| {
                let i = pending[pos]?;
                (self.ready_order[i] != 0
                    && matches!(
                        self.sockets.get::<tcp::Socket>(self.tcp[i]).state(),
                        tcp::State::Established | tcp::State::CloseWait
                    ))
                .then_some((self.ready_order[i], pos, i))
            })
            .min();
        let (_, pos, i) = oldest.ok_or(-11)?;
        self.fds[id][fresh] = Some(Descriptor {
            kind: Kind::Stream(i, Phase::Connected),
            ..d
        });
        pending[pos] = None;
        d.kind = Kind::Listen { pending, limit };
        self.fds[id][n as usize] = Some(d);
        self.replenish_listeners();
        Ok(fresh as u8)
    }
    // Replace an accepted connection with a listening transport when capacity permits.
    fn replenish_listeners(&mut self) {
        for id in 0..CLIENTS {
            for fd in 0..FDS {
                let Some(mut d) = self.fds[id][fd] else {
                    continue;
                };
                let Kind::Listen { mut pending, limit } = d.kind else {
                    continue;
                };
                for item in &mut pending[..limit] {
                    if item.is_none() {
                        *item = self.tcp_slot().ok();
                    }
                    if let Some(i) = *item {
                        if self.tcp_socket(i).state() == tcp::State::Closed {
                            self.fin[i] = false;
                            self.ready_order[i] = 0;
                            self.tcp_socket(i)
                                .listen((LOCAL, d.port))
                                .expect("closed listener transport");
                        }
                    }
                }
                d.kind = Kind::Listen { pending, limit };
                self.fds[id][fd] = Some(d);
            }
        }
    }
    pub fn send(
        &mut self,
        id: usize,
        n: u8,
        addr: Option<IpEndpoint>,
        data: &[u8],
    ) -> Result<usize, i32> {
        let d = self.io_descriptor(id, n)?;
        if !d.wr {
            return Err(-32);
        }
        match d.kind {
            Kind::Datagram(i, peer) => {
                if data.len() > UDP_MAX {
                    return Err(-90);
                }
                let ep = addr.or(peer).ok_or(-89)?;
                remote_ok(ep)?;
                if self.udp_pending[i] >= 4 {
                    return Err(-11);
                }
                if d.port == 0 {
                    self.autobind(id, n)?;
                }
                self.udp_socket(i)
                    .send_slice(data, ep)
                    .map_err(|e| match e {
                        udp::SendError::BufferFull => -11,
                        udp::SendError::Unaddressable => -99,
                    })?;
                self.udp_pending[i] += 1;
                Ok(data.len())
            }
            Kind::Stream(i, Phase::Connected) => {
                if addr.is_some() {
                    return Err(-106);
                }
                if self.tcp_socket(i).state() == tcp::State::Closed && !self.fin[i] {
                    return Err(-104);
                }
                let s = self.tcp_socket(i);
                if !s.may_send() {
                    return Err(-32);
                }
                if data.is_empty() {
                    return Ok(0);
                }
                if !s.can_send() {
                    return Err(-11);
                }
                s.send_slice(data).map_err(|_| -32)
            }
            _ => Err(-107),
        }
    }
    pub fn recv(&mut self, id: usize, n: u8, out: &mut [u8]) -> Result<Received, i32> {
        let d = self.io_descriptor(id, n)?;
        let stream = |count| {
            Ok(Received {
                count,
                full: count,
                peer: None,
            })
        };
        if !d.rd {
            return stream(0);
        }
        match d.kind {
            Kind::Datagram(i, peer) => {
                for _ in 0..4 {
                    let (b, meta) = self.udp_socket(i).recv().map_err(|_| -11)?;
                    if b.len() > UDP_MAX || peer.is_some_and(|p| p != meta.endpoint) {
                        continue;
                    }
                    let count = out.len().min(b.len());
                    out[..count].copy_from_slice(&b[..count]);
                    return Ok(Received {
                        count,
                        full: b.len(),
                        peer: Some(meta.endpoint),
                    });
                }
                Err(-11)
            }
            Kind::Stream(i, Phase::Connected) => {
                if out.is_empty() {
                    return stream(0);
                }
                if self.tcp_socket(i).can_recv() {
                    let n = self.tcp_socket(i).recv_slice(out).map_err(|_| -104)?;
                    return stream(n);
                }
                if self.fin[i] {
                    return stream(0);
                }
                if self.tcp_socket(i).state() == tcp::State::Closed {
                    return Err(-104);
                }
                Err(-11)
            }
            _ => Err(-107),
        }
    }
    pub fn shutdown(&mut self, id: usize, n: u8, how: u8) -> Result<(), i32> {
        if how > 2 {
            return Err(-22);
        }
        let mut d = self.fd(id, n)?;
        match d.kind {
            Kind::Stream(i, Phase::Connected) => {
                if how != 0 {
                    self.tcp_socket(i).close()
                }
            }
            Kind::Datagram(_, Some(_)) => {}
            _ => return Err(-107),
        }
        if how != 1 {
            d.rd = false
        }
        if how != 0 {
            d.wr = false
        }
        self.fds[id][n as usize] = Some(d);
        Ok(())
    }
    fn retire(&mut self, i: usize, abort: bool) {
        if abort {
            self.tcp_socket(i).abort()
        } else {
            self.tcp_socket(i).close()
        }
        self.tcp_usage[i] = Use::Closing;
        self.tcp_retire_at[i] = self.now + Duration::from_secs(2);
    }
    fn remove(&mut self, id: usize, n: u8, dead: bool) -> Result<(), i32> {
        let d = self.fd(id, n)?;
        self.fds[id][n as usize] = None;
        match d.kind {
            Kind::Datagram(i, _) => {
                self.udp_usage[i] = Use::Closing;
                self.retire_at[i] = self.now + Duration::from_secs(2);
            }
            Kind::Stream(i, _) => self.retire(i, dead),
            Kind::Listen { pending, .. } => {
                for i in pending.into_iter().flatten() {
                    self.retire(i, true)
                }
            }
        }
        Ok(())
    }
    pub fn close(&mut self, id: usize, n: u8) -> Result<(), i32> {
        self.remove(id, n, false)
    }
    pub fn exit(&mut self, id: usize) {
        for n in 0..FDS {
            if self.fds[id][n].is_some() {
                let _ = self.remove(id, n as u8, true);
            }
        }
    }

    fn observe(&mut self) {
        for i in 0..TCP {
            if self.tcp_usage[i] == Use::Live
                && self.ready_order[i] == 0
                && matches!(
                    self.tcp_socket(i).state(),
                    tcp::State::Established | tcp::State::CloseWait
                )
            {
                self.ready_order[i] = self.next_order;
                self.next_order = self.next_order.saturating_add(1);
            }
            if self.tcp_usage[i] != Use::Free
                && matches!(
                    self.tcp_socket(i).state(),
                    tcp::State::CloseWait
                        | tcp::State::Closing
                        | tcp::State::LastAck
                        | tcp::State::TimeWait
                )
            {
                self.fin[i] = true;
            }
        }
    }
    fn filter_receives(&mut self) {
        for id in 0..CLIENTS {
            for fd in 0..FDS {
                let Some(d) = self.fds[id][fd] else { continue };
                match d.kind {
                    Kind::Datagram(i, peer) => {
                        for _ in 0..4 {
                            let socket = self.udp_socket(i);
                            let Ok((data, meta)) = socket.peek() else {
                                break;
                            };
                            if d.rd
                                && data.len() <= UDP_MAX
                                && peer.is_none_or(|p| p == meta.endpoint)
                            {
                                break;
                            }
                            let _ = socket.recv();
                        }
                    }
                    Kind::Stream(i, Phase::Connected) if !d.rd => {
                        let socket = self.tcp_socket(i);
                        for _ in 0..2 {
                            if !socket.can_recv() {
                                break;
                            }
                            let _ = socket.recv(|bytes| (bytes.len(), ()));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn poll(&mut self, dev: &mut Frames, now: Instant) {
        self.now = now;
        dev.flush();
        for _ in 0..4 {
            if self.iface.poll_ingress_single(now, dev, &mut self.sockets)
                == PollIngressSingleResult::None
            {
                break;
            }
            self.observe();
            self.filter_receives();
        }
        self.iface.poll_egress(now, dev, &mut self.sockets);
        self.observe();
        while let Some(port) = dev.take_udp_port() {
            for i in 0..UDP {
                if self.udp_usage[i] != Use::Free && self.udp_socket(i).endpoint().port == port {
                    self.udp_pending[i] = self.udp_pending[i].saturating_sub(1);
                    break;
                }
            }
        }
        for i in 0..UDP {
            if self.udp_usage[i] == Use::Closing
                && (self.udp_pending[i] == 0 || now >= self.retire_at[i])
            {
                self.udp_socket(i).close();
                self.udp_pending[i] = 0;
                self.udp_usage[i] = Use::Free;
            }
        }
        for i in 0..TCP {
            if self.tcp_usage[i] != Use::Closing {
                continue;
            }
            let state = self.tcp_socket(i).state();
            if state == tcp::State::Closed && self.tcp_socket(i).remote_endpoint().is_none() {
                self.tcp_usage[i] = Use::Free;
            } else if state != tcp::State::TimeWait && now >= self.tcp_retire_at[i] {
                self.tcp_socket(i).abort();
                if now >= self.tcp_retire_at[i] + Duration::from_secs(2) {
                    self.tcp_usage[i] = Use::Free
                }
            }
        }
        self.replenish_listeners();
    }
    pub fn draining(&mut self) -> bool {
        self.udp_usage.contains(&Use::Closing)
            || (0..TCP).any(|i| {
                self.tcp_usage[i] == Use::Closing
                    && (self.tcp_socket(i).state() != tcp::State::TimeWait
                        && (self.tcp_socket(i).state() != tcp::State::Closed
                            || self.tcp_socket(i).remote_endpoint().is_some()))
            })
    }
    pub fn sleep_ms(&mut self, now: Instant) -> u64 {
        self.iface
            .poll_delay(now, &self.sockets)
            .unwrap_or(Duration::from_millis(2))
            .total_millis()
            .clamp(1, 2)
    }
}
