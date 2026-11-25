# Native socket service / 네이티브 소켓 서비스

CHG-307 follows REQ-024 → DES-013 → IPC-006 → VER-024. IPC-006 has two acceptance groups. Local AF_UNIX behavior runs in an isolated ordinary Rust broker on single-core AArch64 QEMU virt under FIFO or RR. Network acceptance requires real IPv4 TCP/UDP traffic with an independent peer. [IPC-006 #376](https://github.com/Romanof123/JNOX/issues/376) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-307의 IPC-006 인수 범위는 두 그룹이다. 로컬 AF_UNIX는 단일 코어 AArch64 QEMU virt의 FIFO 또는 RR에서 격리된 일반 Rust 브로커가 처리한다. 네트워크 그룹에는 독립 상대와 주고받은 실제 IPv4 TCP·UDP 트래픽이 필요하다.

**IPC-006 local component · IMP-307 · EVD-307.** Independent Release accepted the exercised AF_UNIX service on one emulated Cortex-A72 under FIFO and RR. Independent Release also accepted IPv4 TCP and UDP on this virtual profile. IPC-006 remains open for the local blocking backlog and finite-deadline follow-up in [#381](https://github.com/Romanof123/JNOX/issues/381). [IPv4 evidence](evidence/native-ipv4-socket-release.en-ko.md). [First loader results / 최초 부팅](evidence/native-local-socket-first.en-ko.md) · [Corrected captures / 수정본](evidence/native-local-socket-corrected.en-ko.md) · [Release decision / 출시 판정](evidence/native-local-socket-release.en-ko.md)

**IPC-006 로컬 구성요소 · IMP-307 · EVD-307.** 독립 Release는 에뮬레이션 Cortex-A72 한 코어에서 FIFO·RR로 실행한 AF_UNIX 검증 범위를 승인했다. 독립 배포 검토에서 같은 가상 프로파일의 IPv4 TCP·UDP도 승인했다. IPC-006은 [#381](https://github.com/Romanof123/JNOX/issues/381)의 로컬 차단 연결·기한 만료 후속 판정을 기다린다. [IPv4 검증](evidence/native-ipv4-socket-release.en-ko.md).

The six original socket boots on `e7d3ded` stopped at FAULT 5 before any client exit. Four ordinary socket-disabled control boots were retained. On corrected source `70f2ab812d1d0a444da04c0f8ecdf1f568d533d7`, independent verification completed 12 ordinary boots and 12 traced repeats. Independent Release rebuilt the C server, Rust client and isolated Rust broker for 24 ordinary and eight traced boots. All 32 Release terminal RAM walks found zero socket objects, descriptor entries, queued connections, bytes, records, staged transactions, held tokens and kernel pending exits. The generic scheduler journal dropped up to 256 rows; the indicated broker registers were captured separately.

최초 소켓 부팅 6회는 앱 종료 전에 FAULT 5로 멈췄고, 소켓 비활성 일반 대조 부팅 4회도 보존됐다. 수정 커밋에서 독립 검증은 일반 부팅 12회와 추적 재부팅 12회를 마쳤다. Release는 C 서버·Rust 클라이언트·격리된 Rust 브로커를 다시 빌드해 일반 24회와 추적 8회를 수행했다. Release 부팅 32회 모두 종료 시 소켓 객체·기술자·대기 연결·바이트·레코드·조립 중 트랜잭션·보유 토큰·커널 미처리 종료 건수가 0이었다. 일반 스케줄러 기록은 최대 256행이 누락됐으며 브로커 레지스터는 별도로 수집했다.

V1 accepts C `accept(fd, NULL, NULL)` and Rust `accept(fd)`. C accept with address storage returned ENOTSUP before dequeuing a connection. The eight-object limit returned C errno 23 (ENFILE). The tested C header omitted the ENFILE, ENOENT, EPROTOTYPE and EDESTADDRREQ macros; their SDK correction is tracked separately. Full-backlog connect was observed in nonblocking mode followed by retry. Blocking connect at saturation still needs a guest witness. The ordinary SDK uses an infinite RPC deadline. Raw finite-deadline evidence covers timeout followed by prompt ABORT. Establishing general cancellation requires broker-side evidence of whether a pending operation committed before expiry was detected. These results qualify the documented local subset only.

V1에서 C는 주소 인자를 비운 `accept(fd, NULL, NULL)`, Rust는 `accept(fd)`를 지원한다. C에 주소 버퍼를 주면 대기 연결을 소비하기 전에 ENOTSUP를 반환했다. 객체 8개 한도에서 C errno 23(ENFILE)을 관측했다. 검증한 C 헤더에서 누락된 ENFILE·ENOENT·EPROTOTYPE·EDESTADDRREQ 매크로 수정은 별도로 추적한다. 연결 대기열 포화는 비차단 connect와 재시도로 확인했고, 포화 시 차단 connect 대기는 게스트 관측이 남았다. 일반 SDK의 RPC 기한은 무한이다. 유한 기한 원시 호출은 타임아웃 직후 ABORT한 경우만 확인했다. 일반 취소를 판정하려면 만료를 감지하기 전에 보류 연산이 브로커 상태에 반영됐는지 보여 주는 근거가 필요하다. 이번 승인은 문서화된 로컬 부분집합에 한정된다.

## Setup and identity / 설정과 신원

Package the C and Rust clients in slots 0 and 1 and the broker ELF in slot 2. Add `--socket-service 2 --ipc-route 0:2 --ipc-route 1:2 --peer2-arg 3` to the ordinary three-process package. The broker expects one ASCII digit (1–7), a bitmask of client slots whose exits it must observe before it terminates. Only explicit inbound synchronous routes grant access. Each client uses its own descriptors and transaction buffer. The native `svc 0x4a01` with x8=37 is restricted to the broker. It returns status in x0, a one-shot token in x1, the request word in x2 and the authenticated caller slot in x3. A zero token identifies an exit event for the client in x3. On exit the broker closes that client's state and retries affected blocked operations. [Wire protocol v1](../services/socket/README.md)

일반 C·Rust 클라이언트를 슬롯 0·1, 브로커 ELF를 슬롯 2에 배치할 때 위 옵션을 사용한다. 인자 `3`은 브로커가 종료를 기다릴 클라이언트 0·1의 비트마스크다. 호출 권한은 명시적 동기 IPC 경로로 부여한다. 브로커 전용 SVC 37에서 커널이 호출자 슬롯을 x3에 전달한다. x1의 일회용 토큰이 0이면 해당 슬롯의 종료 사건이다. 브로커는 종료한 클라이언트의 소켓을 정리하고 영향을 받은 대기를 다시 처리한다.

## Local case / 로컬 사례

The service supports named stream, datagram and sequenced-packet sockets. The profile permits eight socket objects, four descriptors per client, two queued connections per listener and 31 pathname bytes. Each stream direction holds 64 bytes. A record queue holds four records of up to 32 bytes each. RPC v1 carries one u64 per call and reply. BEGIN, CHUNK and COMMIT stage one checked operation per authenticated client. META and READ retrieve results and ABORT clears staged state. A nonblocking operation reports pressure immediately. An eligible blocking operation retains its COMMIT reply token until progress or closure.

로컬 이름 기반 스트림·데이터그램·순차 패킷을 지원한다. 소켓 객체는 8개, 클라이언트당 기술자는 4개, 리스너당 대기 연결은 2개, 경로는 31바이트까지다. 스트림은 방향별 64바이트, 레코드 큐는 32바이트 이하 레코드 4개까지다. RPC v1에서 인증된 클라이언트별로 한 트랜잭션을 조립·검사한다. 비차단 요청은 포화를 즉시 알리며, 대기 가능한 차단 요청은 진행 또는 종료 시까지 응답 토큰을 보관한다.

Verify split and coalesced stream reads, partial writes under pressure, half-close and drain-before-EOF. For datagram and seqpacket verify whole-record admission, record boundaries and truncation, and ordering on the connected service. Check descriptor ownership, bounded saturation, peer death and waiter cleanup through ordinary clients.

일반 클라이언트로 스트림 분할·병합 읽기, 포화 시 부분 쓰기, 반쪽 닫기와 잔여 데이터 소진 후 EOF를 확인한다. 데이터그램·순차 패킷은 레코드 단위 수용·경계·잘림 및 연결형 서비스의 순서를 검사한다. 기술자 소유권, 포화, 상대 종료와 대기 해제도 확인한다.

## Independent network peer / 독립 네트워크 상대

Pin the virtual interface, Rust network stack, device features and address profile. Capture actual traffic at an independent peer for a connected TCP stream and peer reset, two distinct UDP datagrams with peer addresses, and observable loss and reordering. Bound buffers and endpoint lifetimes. Record local guest, virtual network and physical evidence separately. IPv4/IPv6 target coverage and product networking remain tracked under #8, with board-specific driver and DMA claims under #34. [Networking #8](https://github.com/Romanof123/JNOX/issues/8) · [Physical board #34](https://github.com/Romanof123/JNOX/issues/34)

가상 인터페이스·Rust 네트워크 스택·장치 기능·주소 프로필을 고정한다. 독립 상대에서 연결된 TCP 스트림과 상대 리셋, 상대 주소를 포함한 UDP 데이터그램 두 개, 실제 손실·재정렬을 기록한다. 버퍼와 종단 수명을 제한하고 로컬 게스트, 가상 네트워크, 실물 관측을 따로 기록한다. IPv4·IPv6 대상 범위와 제품 네트워킹은 #8, 보드별 드라이버·DMA는 #34에서 추적한다.
