# JNOX #376 independent AF_UNIX verification

The corrected candidate `70f2ab812d1d0a444da04c0f8ecdf1f568d533d7` passed the six requested application cases and six supplemental guest cases. Each first attempt cold-booted the captured kernel on a single QEMU virt Cortex-A72 with TCG. A second boot of each identical kernel recorded all broker receive-return and reply-entry registers. Both sets of captures contain UART, raw RAM, ELF and launch provenance. All packages identify a clean candidate tree `b704fbf0af36762d30205ddd3e0a09511325017b`. FIFO and RR used client priorities 7 and 4 and broker priority 1. RR used 625000 ticks.

| Ordinary workload | FIFO and RR result | Calls per boot, slots 0 + 1 |
|---|---|---|
| Stream | C server read 3, 16 and 45 bytes comprising the sequence 1–64. Rust writes returned 7, 57 and −11. The server drained to EOF, replied 32, and the client observed reverse EOF. | 52 + 44 |
| Datagram | The receiver copied [10,11,12] and [20,21], observed original lengths 5 and 2, and saw /origin twice. A further nonblocking receive returned −11. The acknowledgment 74 came from /telemetry. | 35 + 30 |
| Sequenced packet | Four three-byte records arrived in order. A fifth whole-record write returned −11. The server drained the four records to EOF and replied 150. | 47 + 40 |
| Limits and ownership | A second ordinary client received −13 on broker-only receive and −9 on closing another owner's raw fd. The fifth local descriptor returned −24, the ninth object −23 and the third pending connection −11. Freeing resources admitted the third connection and application markers 51/52/53 established accept order. The listener name was 31 bytes. | 54 + 64 |
| Owner death under pressure | Four sequenced packets filled the receiver. A new nonblocking record returned −11. Draining one record completed waiting send token 46 once. The peer exited with four unread records, two descriptors and a partially staged bind. Death notification preceded the one reply of −32 to waiting send token 51. A subsequent receive returned EOF and a new bind reclaimed the peer's name. | 44 + 28 |
| Finite deadline and cancellation | A blocking receive COMMIT with a finite deadline returned kernel −110 while another caller filled and drained four records. The next ABORT cleared the canceled request. Kernel token 7 received no later reply. The new payload 72 arrived exactly once and another nonblocking receive returned −11. | 19 + 49 |

The requested application images were the original C server, Rust client and ordinary Rust broker. All six exited with 73/74/0. The supplemental cases used two instances of the verifier's ordinary C probe with the same broker. Their per-slot exits were 60/61/0, 62/63/0 and 64/65/0. Slots 0 and 1 had explicit routes to slot 2 in every package. Kernel-derived caller registers authenticated both clients and two distinct zero-token death events in every register replay. See `results.json` for both original 12-word arrays, supplemental journals, physical frame addresses and wait token outcomes. The decoder independently checked accepted BEGIN/CHUNK/COMMIT framing per caller, packet payloads and address bytes.

The terminal RAM decoder walked each slot's own three-level page tables and checked private EL0 writable frames. In all 24 captures the eight objects, twelve descriptor entries, connection/stream/record queues and broker transaction state were free. All three kernel IPC slots were retired and no pending request, receiver, completion or exit bit remained. Unlinked AArch64 type witnesses supplied Rust field offsets and enum discriminants. The checker validates their aggregate sizes against the captured release symbols. Four socket engine and six kernel synchronous IPC Rust host tests passed.

The UART scheduler log retained 128 events per boot. It dropped 73–233 later events across the twelve cases. Register observations came from separate `-singlestep` boots of the same kernels. The register records validate causal ordering and one-shot outcomes. Wall-clock performance was outside this run. IPv4 TCP/UDP, independent network peer traffic, physical hardware and the overall IPC-006 verdict require separate evidence.

The corrected revision's immediate parent `e7d3ded373b58df1a8f520d6a3d3aa3d418eef2a` failed all six boots at loader FAULT 5 before application exit. The original sealed first-attempt packet and its hash are under `history/`. The corrected attempts remained separate. The supporting files retain the first offline layout witness whose release ELF lacked DWARF and the unlinked object used to read the types successfully.

## Replay

After extracting the archive on Linux, run `sha256sum -c SHA256SUMS`, followed by `python3 verify.py .`. Offline verification needs Python 3 and GNU AArch64 binutils. The result is reproducible from the archived evidence. To cold-boot the kernels, run `python3 replay.py . /absolute/path/to/a/new-directory` with QEMU system AArch64 available. The original captures used QEMU 6.2.0. The frozen build sources, tool versions, first build scripts, exact ELF inputs, package commands, broker register collection and hashes accompany the archive. The issue text was the supplied snapshot and the contract was read from documentation commit `559c91ee76cd7ff133ee1a3cf7ec3942afdd5197`.

## 한국어

수정본 `70f2ab8`의 AF_UNIX 스트림·데이터그램·순차 패킷을 FIFO와 RR에서 각각 독립 부팅했다. 일반 C 서버, Rust 클라이언트, 격리된 Rust 브로커는 여섯 경우 모두 73/74/0으로 종료했다. 스트림은 쓰기 7·57·−11과 읽기 3·16·45, 1–64 바이트의 합 2080 및 EOF를 확인했다. 데이터그램은 두 레코드의 원래 길이 5·2와 잘린 내용, 양쪽 송신 경로를 확인했다. 순차 패킷은 3바이트 레코드 네 개의 순서, 다섯 번째 쓰기 거부, EOF와 응답 150을 확인했다.

추가 여섯 부팅에서는 타 소유자 기술자 거부, 브로커 전용 호출 거부, 기술자 4개·객체 8개·대기 연결 2개의 포화와 재사용을 확인했다. 레코드 큐가 가득 찬 상태에서 토큰 46의 쓰기는 한 칸이 비자 한 번 성공했고, 토큰 51의 쓰기는 상대가 종료한 뒤 한 번 −32를 받았다. 종료한 상대가 사용하던 경로를 재사용했다. 유한 기한 수신은 −110을 받았으며, 취소된 토큰 7에 재응답하지 않았다. 다음 데이터는 한 번만 도착했다.

첫 실행 12회와 동일 커널의 레지스터 추적 재실행 12회 모두 슬롯별 페이지 테이블로 최종 RAM을 해석했다. 소켓 객체와 기술자, 연결·바이트·레코드 큐 및 남은 응답 토큰은 모두 비어 있었다. UART 스케줄러 기록은 128개까지 보존되어 이후 73–233개가 생략되었다. 호출자와 종료 사건은 별도 재실행의 실제 브로커 레지스터로 확인했다. 이전 판본의 여섯 번 부팅 실패 원본도 보존했다. 실제 IPv4 상대 트래픽, 실물 하드웨어 및 전체 IPC-006 승인은 별도 검증 대상이다.
