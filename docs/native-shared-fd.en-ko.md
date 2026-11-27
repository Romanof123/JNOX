# CHG-337 · Shared backing descriptors for local sockets / 로컬 소켓으로 공유하는 메모리 FD

REQ-024 · DES-013 · VER-024. The display consumer and the producer run in separate address spaces. A Wayland client passes a file descriptor and a pool size to the compositor to create a shared-memory pool. This first native transport provides an object both processes can map. The wire protocol, compositor and Xwayland still need their own guest runs. [Change #444](https://github.com/Romanof123/JNOX/issues/444) · [Display roadmap #412](https://github.com/Romanof123/JNOX/issues/412) · [Wayland shared-memory protocol](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_shm)

## Requirement and design

* A routed client can create one zero-filled backing of 1 to 307,200 bytes per call. Four global objects, eight local FDs per process, four independent mappings per process and eight queued attachments bound resource use on the single AArch64 boot CPU. The package enables the facility with `--shared-fd` and names its AF_UNIX broker and incoming client routes.
* An FD, a live mapping and an attachment queued for delivery each hold an independent reference. The sender may close and reuse its numeric descriptor immediately after a successful send. Receiving installs a free descriptor that refers to the same backing and access mode. A client may derive a read-only FD and pass it while keeping a writable mapping. EL1 rejects a writable mapping through the read-only FD. Every user mapping is non-executable.
* A stream send with one FD attaches its right to the first committed payload byte. A receive returns any ordinary prefix before the next attachment. At its first byte the broker can install one FD, or discard the reference if plain receive or insufficient control space was requested. Truncated control sets `MSG_CTRUNC` for `recvmsg`. Each transaction uses the authenticated IPC caller and the live COMMIT reply token. Cancellation before successful reply preserves both bytes and rights. Socket shutdown, peer death and broker exit reclaim undelivered references.
* The producer and consumer use the C or Rust SDK and cannot name physical pages. EL1 checks process identity, local descriptor and access mode, installs per-process page table entries, and invalidates stale translations on unmap. The broker is trusted to select the stream peer. Bringing unrelated drivers, DMA buffers, generic filesystem FDs or arbitrary Linux binaries into this profile requires further work. [SDK contract](../sdk/native-aarch64/local-socket.md) · [Broker contract](../services/socket/README.md)

## Acceptance

The guest fixture uses a producer, a display consumer, an unrelated client and the socket broker. Capture the sent right while both source FDs refer to a new object, then capture its installation. Walk both page tables to the same 75 backing pages, check the receiver's read-only permission and compare all 320 × 240 pixels externally before and after the producer edits through its retained mapping. After shutdown inspect real descriptor, mapping, escrow and page-table state. Exercise truncation, cancellation, unrelated access and ordinary byte-only regression. Preserve the package, command line, guest memory, display captures and independent replay before attaching an evidence record. [Previous display acceptance](native-window-pair.en-ko.md) · [Trace graph](../trace/links.csv)

## 한국어 설계와 수용 기준

생성한 앱의 FD, 각 앱의 매핑, 아직 수신하지 않은 소켓 데이터에 붙은 권한을 각각 추적한다. 송신자가 FD를 닫고 같은 번호로 새 객체를 만들어도 수신자는 처음 보낸 객체를 얻어야 한다. 송신자는 쓰기 매핑을 유지하면서 읽기 전용 FD를 만들어 보낼 수 있다. 수신자의 쓰기 매핑 요청은 커널이 거부한다. 커널이 살아 있는 호출의 신원과 응답 토큰을 확인한 뒤 전송 또는 설치를 확정한다. 데이터보다 먼저 받기, 부분 읽기, 제어 공간 부족, 호출 취소와 프로세스 종료에서도 객체 수명이 일관돼야 한다.

두 앱의 실제 페이지 테이블과 접근 비트, 전송 중 권한, 수신자 전용 FD를 정지된 게스트에서 읽는다. 송신자가 픽셀을 갱신하기 전후로 공유 페이지와 화면 전체를 대조한다. 종료 후 미수신 권한과 매핑의 해제를 확인한다. 이 변경의 검증 범위는 지정 가상 장치의 공유 메모리와 FD 전달이며, Wayland 합성기와 Xwayland 실행은 상위 계획에서 별도로 검증한다.

EVD-337 records two cold guest runs and an ordinary byte-socket window regression, including scanouts, reconstructed RAM and independent replay. 두 번의 부팅과 기존 창 시험 회귀에서 공유 메모리·페이지 테이블·화면을 확인했다. [Measured results and captures / 실측과 원본](evidence/native-shared-fd/README.md)
