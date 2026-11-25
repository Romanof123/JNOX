# IPC-006 IPv4 release verification / IPv4 출시 검증

**Support acceptance of the native IPv4 TCP/UDP group on single-core AArch64 QEMU virt under FIFO and RR.** Four fresh cold boots used the public corrected code at `874720752f02d52788ecc03696209332045731af`. All four ran an ordinary C TCP executable in slot 0, a separate ordinary Rust UDP executable in slot 1 and a separate Rust smoltcp broker in slot 2. The independent peer used normal Linux sockets inside a private network namespace. The two TAP capture positions and the applications’ private memory agree on the traffic each application handled.

**승인 범위.** 공개 수정 커밋을 빌드한 단일 Cortex-A72 QEMU virt의 FIFO·RR에서 네이티브 IPv4 네트워크 그룹 승인을 지지한다. 네 번의 콜드 부팅 모두 C TCP 앱·Rust UDP 앱·Rust 서비스가 75·76·0으로 종료했다. 독립 Linux 피어의 소켓 기록·양쪽 이더넷 캡처·게스트 각 프로세스의 전용 RAM을 대조했다.

| New cold position | TCP/UDP/broker exits | Device RX | TX submitted/completed | Device faults | TCP/UDP packets at each capture |
|---|---|---:|---:|---:|---:|
| Public examples, FIFO | 75 / 76 / 0 | 19 | 18 / 18 | 0 | 34 |
| Public examples, RR | 75 / 76 / 0 | 19 | 18 / 18 | 0 | 34 |
| Two-source UDP records, FIFO | 75 / 76 / 0 | 18 | 17 / 17 | 0 | 32 |
| Two-source UDP records, RR | 75 / 76 / 0 | 17 | 17 / 17 | 0 | 32 |

All four packages use priorities 4/4/7, explicit IPC routes 0→2 and 1→2, and a broker mask of 3. The RR quantum is 625000 ticks. The launch records identify QEMU 6.2 TCG, one Cortex-A72, 128 MiB, modern virtio-net MMIO and a namespace-owned TAP. The namespace contained only loopback and the TAP with a connected IPv4 /24 and no default route. The new record fixture assigned a second local IPv4 address to the same TAP. Its two independently bound source sockets belong to the same Linux peer.

## Application observations / 애플리케이션 관측

On every boot the public C application initiated a TCP connection, sent `GUEST/out`, read all 13 bytes of `HOST/outbound` across short reads and observed EOF. Its listener accepted the peer's independent connection, read `HOST/inbound` (12 bytes), observed EOF and sent `GUEST/accepted` (14 bytes). Each of these two flows carried FINs from both endpoints. On a third connection the peer received `ARM!` and used ordinary abortive socket close. Both captures contain the following host-to-guest RST, and the C process recorded `recv = -1`, `errno = 104` (ECONNRESET) and successful close. The private result was `[4,9,13,3,0,12,14,0,4,-1,104,1]` in all four boots.

In both public-example boots, the peer received four distinct outbound UDP records and later `DONE`. The private Rust result and both captures give reply IDs 1,4,3,0 from 10.0.2.2 port 43004. The Rust app completed nine empty nonblocking polls after the end marker. The public example omits ID 2 when composing replies. Independent redecoding of the earlier controlled Ethernet relay captures under both policies confirms that the independent peer actually transmitted 1,2,3,4,0, the relay dropped frame 2 and delayed frame 3, and the QEMU device received the four complete surviving Ethernet frames byte-for-byte in order 1,4,3,0. Both archived host captures and all four fresh host captures recorded zero kernel capture drops.

The additional Rust application issued ordinary `send_to4` and `recv_from4` calls with the public SDK. Each transmitted message and response appears at both Ethernet positions and in the Linux socket journal. Returned counts, payload prefixes and sender addresses came independently from the Rust process's post-stop private memory.

| UDP operation in each additional boot | Observed application result |
|---|---|
| Guest sends an empty record, peer replies with an empty record | Both send and receive returned 0. Guest received source 10.0.2.2 port 43004. The three buffered bytes remained 0xa5. |
| Peer sends `abcdefgh` from 10.0.2.3 port 43005 | Three-byte buffer returned 3 and `abc`, with this exact source. |
| Peer sends `UVWXYZ` from 10.0.2.2 port 43004 | Three-byte buffer with MSG_TRUNC returned 6 and `UVW`, with this exact source. |
| Immediately receive again using MSG_DONTWAIT | EAGAIN (−11), then `DONE` sent all four bytes and close succeeded. |

The receive fixture verified the guard bytes on both sides of its three-byte buffer after every call. Two zero-length frames have UDP length 8 on the wire. Subsequent receives verified datagram boundaries and the discard of truncated suffixes. FIFO and RR recorded identical application return values. The two-sided comparison checks the full TCP/UDP packet multiset, the UDP sequence and each TCP flow's direction, payload and termination. One unrelated TCP packet and one UDP packet crossed in the opposite capture order during the additional FIFO run. The preserved verifier diagnostic records this cross-protocol timing difference. Each flow retained its own order.

**결과.** 네 부팅 모두 정상 TCP 두 연결에서 양방향 FIN·EOF 0, 세 번째 연결에서는 `ARM!` 뒤 RST와 앱의 −1/104를 확인했다. 기존 양 정책 링크 캡처에서 상대 송신은 1·2·3·4·0, 장치와 앱의 수신은 1·4·3·0이었다. 추가 사례에서는 빈 UDP 레코드 양방향 송수신, 두 송신지의 정확한 주소·포트, 3바이트 버퍼에서 일반 수신 3/`abc`와 MSG_TRUNC 수신 6/`UVW`, 그 뒤 EAGAIN과 정상 종료를 확인했다.

## Provenance, first results and replay / 원본과 재현

The fetched PR evidence head was `3134700401c4c09a04b1b8d6a9df09e3fb8fd71a`. Git blob identities for both published archives matched the supplied local files. Fresh extraction verified every inner SHA-256 (764 entries in the original failure and 818 in the corrected archive). Each archive's regenerated JSON exactly matched its retained replay. The original ARP-stage failure on the previous candidate remains recorded, including 103/113/85 and host timeouts. The corrected archive retains fourteen boots covering six traffic positions plus resources and selector/permission controls. Those eight control boots are covered here by fresh extraction and replay. The four new cold boots cover application traffic.

The absolute Git revisions, full ELF hashes, original/corrected archive digests, tool versions, individual kernel manifests, separate slot ELFs, all four new RAM snapshots, peer journals, UART and raw captures are included. Independent replay walks the three TTBR0 page-table roots located via the linked kernel and verifies separate private user-writable pages for the named application and broker result symbols. Launch, manifest and UART bind the executed ELF identities. A public clean build produced the same C TCP executable hash as the archived local build. The public build's Rust UDP and broker ELF hashes differ from that archive. Each capture is verified against its own executable. The tested source comparison shows the additional historical IPC example, C errno constants and a trace ledger update. The two commits after the tested public code add evidence and guide changes.

For offline replay on Linux with Python 3 and GNU `aarch64-linux-gnu-nm`, extract the supplement into an empty directory and run:

```sh
sha256sum -c SHA256SUMS
python3 release-replay.py > /tmp/jnox-ipv4-release-replay.json
cmp replay.json /tmp/jnox-ipv4-release-replay.json
```

Physical NIC, driver, DMA/cache coherence, non-virt boards, multicore, IPv6, Linux binary ABI, sustained load and production networking remain separate profiles. This supplement addresses the IPv4 group of #376. The AF_UNIX group and #24 synchronous IPC acceptance have separate records.

**한계·재현.** 실제 보드/NIC의 DMA·캐시 일관성, 멀티코어, IPv6, Linux 바이너리 ABI, 장시간 부하·제품 네트워크는 별도 검증 대상이다. 이번 판정은 #376 IPv4 그룹에 한정한다. 원 실패 자료와 수정 자료의 해시 및 전체 재생 결과를 각각 다시 확인했다.

[Release packet and replay / 배포 검산 원본](native-ipv4-socket-release-supplement.tar.gz). SHA-256 `ff2eee44c845006306385b3d3a5c9dcf2825e001c5083af261c1a6fa323b015d`.
