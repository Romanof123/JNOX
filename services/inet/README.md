# Native IPv4 socket service

Build this ordinary EL0 service with the native Rust SDK.

```sh
sh sdk/native-aarch64/build-rust.sh services/inet/Cargo.toml inet.elf
```

The broker takes one ASCII digit giving its authorized-client slot mask. Configure inbound synchronous IPC routes, a socket service slot and the matching virtio-net service slot in the native package. SVC 40 polls authenticated request or exit events. SVC 23 consumes each reply token. Only this process uses SVC 38 and 39 for Ethernet frames. The guest profile selects QEMU virt, non-transitional virtio-net, MAC `52:54:00:12:34:56`, IPv4 `10.0.2.15/24`, gateway `10.0.2.2`, and Ethernet frames up to 1514 bytes.

The lock file pins smoltcp 0.12.0 and its transitive dependencies. The four enabled features are `medium-ethernet`, `proto-ipv4`, `socket-tcp` and `socket-udp`. Default features are disabled. The interface processes real ARP, IPv4 and transport traffic through the kernel frame operations. Buffer ownership, endpoint state and transport timers reside in the service address space. [smoltcp 0.12.0](https://docs.rs/smoltcp/0.12.0/smoltcp/)

The request frames follow the local socket v1 BEGIN/CHUNK/COMMIT/META/READ/ABORT format. An IPv4 endpoint is six bytes containing the network-order port and four IPv4 octets. Bind accepts the configured local address or INADDR_ANY. Port zero selects a free port from 49152–49183. Blocking connect, accept, send and receive retain the COMMIT token on EAGAIN. Each client can have one incomplete transaction or retained token.

| Opcode | Arguments and result |
| --- | --- |
| 10 socket | Kind 1 TCP or 2 UDP, returns a caller-private descriptor index 0–3 |
| 11 bind, 12 connect | Six-byte endpoint. TCP connect completes the handshake or reports refusal/timeout. An in-progress nonblocking connect returns EAGAIN. |
| 4 listen, 5 accept | Backlog 1 or 2 on a bound TCP socket. Accept returns a new descriptor after a real handshake. |
| 13 send | Optional six-byte UDP destination followed by data. Connected UDP can use an empty destination. Stream requests carry up to 64 bytes and return the admitted count. Datagram sends accept one whole record up to 32 bytes. |
| 14 receive | Capacity 0–64. COMMIT returns the copied count. META returns copied count, original record length and source length. READ yields the bytes followed by a six-byte UDP source. A stream has zero source length. |
| 8 shutdown, 9 close | Shut read, write or both using argument 0, 1 or 2. TCP write shutdown sends FIN after queued bytes. Reads drain before EOF. An established socket reset before FIN reports ECONNRESET when the receive buffer is empty. |

Six TCP transports each reserve 1024 receive bytes and 128 transmit bytes. Listener backlog entries and accepted streams use this same pool. Four UDP transports each reserve four metadata entries and 128 payload bytes per direction. The wire profile admits four pending datagrams of up to 32 bytes per socket. Oversized incoming records and datagrams from another peer on a connected UDP descriptor are discarded whole. Kernel TX backpressure retains one entire Ethernet frame. A closing UDP socket drains committed records through actual device acceptance or expires after two seconds. A closing TCP socket gets two seconds for graceful teardown, followed by an abort attempt. TCP TIME-WAIT follows smoltcp's timer. Active connect has a 15-second broker deadline. The final service drain lasts at most two seconds after the last authorized process exits.

Per iteration the service polls up to four inbound frames, one bounded smoltcp transmit pass, its existing pending calls, and one new RPC. An idle service sleeps for 1–2 ms on the native monotonic clock. Process exit clears that owner's partial requests and descriptors. TCP/UDP buffers, Ethernet staging buffers and the per-caller frame state use static, process-private storage.
