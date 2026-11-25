# Native socket service / 네이티브 소켓 서비스

CHG-307 follows REQ-024 → DES-013 → IPC-006 → VER-024. IPC-006 has two acceptance groups. Local AF_UNIX behavior runs in an isolated ordinary Rust broker on single-core AArch64 QEMU virt under FIFO or RR. Network acceptance requires real IPv4 TCP/UDP traffic with an independent peer. The local implementation awaits independent results at this revision. Both groups remain open. [IPC-006 #376](https://github.com/Romanof123/JNOX/issues/376) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-307의 IPC-006 인수 범위는 두 그룹이다. 로컬 AF_UNIX는 단일 코어 AArch64 QEMU virt의 FIFO 또는 RR에서 격리된 일반 Rust 브로커가 처리한다. 네트워크 그룹에는 독립 상대와 주고받은 실제 IPv4 TCP·UDP 트래픽이 필요하다. 이 판본에서는 로컬 구현의 독립 실행 결과를 기다리고 있으며 두 그룹 모두 승인 전이다.

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
