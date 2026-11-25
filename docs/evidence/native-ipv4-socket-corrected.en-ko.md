# Independent IPv4 socket verification / IPv4 소켓 독립 검증

**PASS for the #376 IPv4 QEMU group at `05b26e7407f1901e9213280846f295b12d681024`.** The documented ordinary AArch64 C TCP application, Rust UDP application and Rust smoltcp service ran from separate ELFs in slots 0, 1 and 2. All six traffic runs exited 75, 76 and 0. The remaining eight boots exercised resource, permission and feature-selection controls. The evidence covers a single Cortex-A72 under QEMU 6.2 TCG with modern virtio-net MMIO. Hardware DMA/coherence, physical networking, other boards, IPv6 and sibling local IPC acceptance have separate verification scopes.

**판정.** 수정 커밋의 단일 코어 QEMU에서 #376 IPv4 네트워크 그룹을 통과했다. FIFO·RR 각각 루프백, 직접 연결한 TAP, 프레임 유실·재정렬을 주입한 TAP에서 C TCP 앱, Rust UDP 앱, Rust 브로커가 75·76·0으로 종료했다. 나머지 여덟 부팅에서는 자원 상한과 권한·기능 선택을 검증했다.

| Network position | Policy | Kernel RX | Kernel TX / completed | Guest empty polls after marker |
| --- | --- | ---: | ---: | ---: |
| Loopback SLIRP | FIFO | 20 | 18 / 18 | 9 |
| Loopback SLIRP | RR | 20 | 18 / 18 | 9 |
| Direct isolated TAP | FIFO | 18 | 18 / 18 | 9 |
| Direct isolated TAP | RR | 18 | 18 / 18 | 9 |
| TAP with controlled Ethernet relay | FIFO | 18 | 19 / 19 | 9 |
| TAP with controlled Ethernet relay | RR | 18 | 19 / 19 | 9 |

Each traffic position had zero device faults. The private broker activity reports record both client exits, positive RPC and idle counts. Page-table replay checks the private writable mappings for each app receipt and the broker activity counters. Kernel snapshots show eight descriptors per direction in 4 KiB-aligned kernel queue storage, descriptor ranges inside queue storage and TX ring indices equal to the completed UART count. The trace event stream records its own bounded-buffer drops. TCP/UDP conclusions use original captures, peer process receipts, ELF identity and per-process RAM.

## TCP and UDP

The C program sent `GUEST/out`, consumed 13 bytes of `HOST/outbound` in reads of at most three, then observed EOF. It accepted an independent connection, read 12 bytes of `HOST/inbound` in reads of at most four, observed EOF and sent `GUEST/accepted`. The peer received both transfers and read EOF. The two orderly TCP flows have FINs in both directions. A separate flow carried `ARM!`, then a guest-facing RST from the host. Private RAM recorded `recv=-1, errno=104` and successful close. The C result was `[4,9,13,3,0,12,14,0,4,-1,104,1]` in all six runs.

The Rust program sent four distinct three-byte records from 10.0.2.15:43003 and validated the source 10.0.2.2:43004 and bytes on every returned record. All six runs delivered IDs 1,4,3,0. Nine empty nonblocking receives spanned the 100 ms guest-time observation window. The peer then received `DONE`, and the Rust app closed the descriptor.

In the original peer fixture the relay received all four guest records and omitted ID 2 when composing its replies. The additional Ethernet relay positions exercised an already-transmitted guest-bound datagram. The ordinary host peer sent replies 1,2,3,4,0. Tcpdump on Linux jnx0 captured all five Ethernet frames. A separate relay forwarded frame 1, discarded 2, held 3, forwarded 4, then released 3 and forwarded marker 0. The QEMU device pcap contained the four exact surviving Ethernet frames in the order 1,4,3,0. Both Linux captures recorded zero kernel capture drops. The relay forwards full frames without modifying them. Its loopback QEMU socket and TAP ran in a fresh Linux namespace containing only lo, jnx0 and the connected 10.0.2.0/24 route.

The two direct TAP filter-dumps contain per-record virtio headers of 10 or 12 bytes before Ethernet despite the pcap Ethernet link type. Replay identifies the offset before decoding ARP, IPv4, TCP and UDP. Loopback, Ethernet-relay QEMU captures and Linux tcpdump use offset zero. Independent endpoint logs record the host-translated source port in SLIRP and guest port 43003 in TAP positions.

**TCP·UDP.** 두 정상 TCP 연결에서 양방향 FIN과 앱의 EOF 0을 확인했다. 별도 연결에서는 ARM! 수신 후 호스트가 RST를 보내고 게스트가 -1과 errno 104를 기록했다. UDP 독립 피어가 실제 전송한 1·2·3·4·0 다섯 프레임이 리눅스 TAP 캡처에 남았다. 중간 링크에서 2를 버리고 3을 보류한 뒤 4 다음에 전달했다. 게스트 쪽 캡처와 Rust 앱 결과 모두 1·4·3·0이었다. 생존 프레임 네 개의 전체 바이트가 링크 양쪽에서 일치했고, 양 정책 모두 호스트 캡처 드롭은 0건이었다.

## Resource limits and controls

In each policy the independent C resource fixture opened four private UDP descriptors. A fifth returned EMFILE (24). A second ordinary caller received ENFILE (23) when four UDP transports were occupied and EBADF (9) when closing an unowned local handle. Duplicate bind returned EADDRINUSE (98) and an outgoing 33-byte UDP record returned EMSGSIZE (90). Closing the owner's UDP sockets let the second caller open one. Six simultaneous TCP transports across the two callers exhausted the TCP pool and the seventh returned ENFILE (23). The owner exited with four sockets. The remaining caller then obtained two more TCP transports, reached its own four-descriptor limit and closed all four. The clients and broker exited 81, 82 and 0. The same fixture checked unsupported IPv6-domain number 10, TCP socket with UDP protocol and IPv4 sequenced-packet requests (95), plus AF_UNIX against the IPv4-only service (22).

Six two-slot control boots covered enabled, local-only and plain kernels under FIFO/RR. The enabled unselected caller received -13 for raw calls 38/39/40. The selected slot saw -11 on empty receive and try-receive, and -90 for frame sizes outside the profile. Both local-only and plain profiles returned -38 to both slots on all three calls.

**자원·권한.** 프로세스당 네 번째 이후 디스크립터 요청은 24, UDP 네 개와 TCP 여섯 개 점유 뒤 다른 프로세스의 추가 요청은 23이었다. 중복 바인드는 98, 33바이트 UDP 송신은 90, 소유하지 않은 핸들 닫기는 9를 반환했다. 소유 프로세스가 TCP 소켓 네 개를 열어 둔 채 종료한 후 다른 프로세스가 회수된 자원을 사용했다. 두 정책에서 선택 슬롯의 빈 큐와 잘못된 길이, 비인가 슬롯 거부, 로컬 전용·기본 프로필의 미지원 반환값을 확인했다.

## Provenance and replay

The immediate parent `906fdd07e36fa9e2e6d53ba5aee412bd31e580b4` timed out at the host peer and exited 103/113/85 after the first successful ARP transmission. The retained previous report, parsed results and archive digest are in `previous/`. Its full sibling archive is [the first IPv4 capture packet](native-ipv4-socket-first.tar.gz), SHA-256 `d4cf67f8d285b3a0125199ffe4d25cfc850f283c31fe8ee9725bcdf9a46e5c6e`. The single candidate change marks x1 read-write around the Ethernet frame SVC. The first outcomes of the corrected executable positions are in the respective capture directories.

The packaged workload used the example priorities 4/4/7 and RR quantum 625000. Build and run scripts preserve observed commands. The additional IPC route 1:0 in the resource fixture coordinates its two ordinary clients. The controlled-link host fixture changes the QEMU backend and host send sequence. All six traffic positions use identical application and broker ELF hashes. Raw artifacts include 14 full 128 MiB RAM images, package manifests, kernel and user ELFs, QEMU argv, UART, application peer journals and eight raw pcaps. The archive also includes the tested sources and capture/replay scripts.

After verifying the archive SHA-256, extract into a new directory, then run `sha256sum -c SHA256SUMS` and `python3 replay.py` there. Python 3, pyelftools and GNU aarch64-linux-gnu-nm are needed. Compare the output with `replay.json`. The clean extraction receipt records a separate check of every file and a byte-identical replay.

**재현.** 이전 후보의 원본 실패와 진단은 별도 보존했다. 이번 패키지에는 콜드 부팅 14회의 RAM, 원본 pcap 8개, ELF, 매니페스트, UART, 독립 피어 로그와 검사 스크립트가 있다. 압축 파일 해시를 확인하고 새 디렉터리에 풀어 각 파일 해시와 replay.json을 대조할 수 있다.

[Download packet captures, ELFs and replay / 패킷·실행 파일 및 재현 도구](native-ipv4-socket-corrected.tar.gz). SHA-256 `55b524ec7425b4b831d44422896ee4deaa259fcde2e23c9e2f5749ce68fc4a9e`.
