# IPC-001 · Native synchronous call, receive and reply / 네이티브 동기 호출·수신·회신

CHG-302 authorizes implementation on the single-core AArch64 QEMU virt FIFO/RR configuration verified under #363. The owner opened #24 with 0/10 families accepted. The scheduling release stands at 6/7. Physical Pi 4 evidence and applicable product timing and safety approvals remain open. The ten original family acceptance cases still apply. [Owner decision #365](https://github.com/Romanof123/JNOX/issues/365) · [Virtual playback #363](https://github.com/Romanof123/JNOX/issues/363) · [Scheduling #21](https://github.com/Romanof123/JNOX/issues/21) · [Physical acquisition #34](https://github.com/Romanof123/JNOX/issues/34) · [IPC tracker #24](https://github.com/Romanof123/JNOX/issues/24) · [Original cases](ipc-mechanisms.en.md)

CHG-302에 따라 #363에서 검증한 단일 코어 AArch64 QEMU virt FIFO·RR에서 IPC 구현을 시작한다. [#24](https://github.com/Romanof123/JNOX/issues/24) 착수 당시 IPC 계열 승인 수는 0/10이었고, #21 스케줄링 출시는 6/7이다. #34 Pi 4 실물 계측과 해당 제품 타이밍·안전 승인은 미결이다. 열 개 IPC 계열의 기존 인수 사례를 유지한다. [최초 인수 사례](ipc-mechanisms.md)

## Endpoint and ABI / 엔드포인트와 호출 규약

Each configured process owns its slot-numbered endpoint, including slot 0. Each repeated package option `--ipc-route CALLER:SERVER` grants only the indicated direction between configured slots. RECEIVE uses the current process's endpoint. A request and a reply each carry one copied 64-bit word in registers across private address spaces. With no routes, existing fixed profiles retain their default syscall behavior.

각 프로세스는 자신의 슬롯 번호(0 포함)에 해당하는 엔드포인트를 소유한다. 반복 지정하는 `--ipc-route CALLER:SERVER`는 해당 슬롯 사이의 한 방향 호출만 허용한다. 수신은 현재 프로세스 소유의 엔드포인트에서만 가능하다. 요청과 회신은 각각 64비트 한 워드를 레지스터로 복사하고 사용자 포인터는 전달하지 않는다. 경로가 없는 고정 정책 패키지는 기존 시스템 호출 동작을 유지한다.

| native-v1 SVC 0x4a01 | Input / 입력 | Result / 결과 |
| --- | --- | --- |
| CALL, x8=21 | x0 server slot, x1 request word, x2 absolute monotonic deadline in ns or UINT64_MAX for no expiry | x0 status, x1 reply on success / 상태와 성공 시 회신 |
| RECEIVE, x8=22 | Current endpoint / 현재 엔드포인트 | x0 status, x1 single-use token, x2 request word on success / 성공 시 일회용 토큰과 요청 |
| REPLY, x8=23 | x0 token, x1 reply word / 토큰과 회신 | x0 status / 상태 |

## State and scheduling / 상태와 스케줄링

Validate routes, endpoint authority, token owner and generation before changing state. Keep one bounded call record per caller and FIFO arrival order at each endpoint. Block the caller until exactly one reply, timeout or peer-loss outcome wins. A live token names the accepted transaction and its receiving server. A foreign, reused or expired token leaves current messages, queues and waiters intact. Serialize trap and timer transitions with IRQs masked. Process a due absolute deadline before a reply at the same instant. On exit, detach queued and accepted calls involving the departing peer, invalidate tokens, reclaim slots and wake surviving waiters once.

Donate the blocked caller's effective priority along authorized active calls. Detect a cycle before enrolling the new wait. On every terminal path, withdraw that edge and recompute effective ranks from base priorities and surviving donors. Preserve FIFO/RR selection and accounting for runnable peers at their resulting ranks.

상태 변경 전에 호출 경로·엔드포인트 권한·토큰 소유자와 세대를 확인한다. 호출자별 유한 상태와 엔드포인트별 FIFO 도착 순서를 유지한다. 회신·만료·상대 종료 중 하나가 확정될 때까지 호출자를 차단한다. 수신 서버만 현재 거래의 토큰을 한 번 쓸 수 있다. 타인·재사용·만료 토큰 거부 시 현재 메시지·대기 상태를 보존한다. 인터럽트를 마스킹한 예외·타이머 경로에서 전이를 직렬화하고 같은 시각에는 절대 기한 만료를 먼저 처리한다. 상대 종료 시 대기·수신된 호출을 분리하고 토큰을 무효화해 슬롯을 회수하며 생존 대기자를 한 번만 깨운다.

호출자의 유효 우선순위를 허가된 활성 호출 체인에 전파한다. 순환은 대기 등록 전에 거부한다. 모든 종료 경로에서 해당 기부를 회수하고 기본 우선순위와 남은 기부자로부터 유효 순위를 다시 계산한다. 해당 순위의 실행 가능 작업에는 기존 FIFO·RR 선택과 회계를 적용한다.

## Candidate workload and acceptance / 후보 작업과 인수

Implement the package route, kernel transitions and C/Rust SDK calls. Build separate native-v1 ELFs from a C caller and Rust server. The caller sends a sample word; the server computes a reply from the received value, and the caller verifies it in private memory. Exercise both arrival orders. Add a runnable fixed medium-priority worker between high caller and low server. Observe caller block, donated server service ahead of medium, caller resumption with the reply, and medium progress after withdrawal in FIFO and RR.

Exercise expiry before receive and after token issuance, wrong-owner and duplicate replies, stale tokens after slot reuse, caller loss and server loss. Check one terminal status, restored priorities, recovered capacity and a subsequent successful call. Use independent host cases for exact-time reply/expiry, cycles and cleanup races that cannot be forced in a guest. Retain native products, syscall results, owner switches and independent replay under VER-024. Record observed cases before changing the #24 checklist. [REQ-024](requirements.en.md) · [DES-013](architecture.en.md) · [VER-024](verification-plan.en.md)

패키지 권한 경로, 커널 상태 전이와 C·Rust SDK 호출을 구현한다. 별도 소스에서 빌드한 C 호출자와 Rust 서버는 한 워드를 교환한다. 서버가 수신값으로 회신을 계산하고 호출자가 사설 메모리에서 결과를 확인한다. 호출자·서버가 각각 먼저 도착하는 두 순서를 실행한다. 높은 호출자와 낮은 서버 사이에 고정 중간 우선순위 작업을 둔다. FIFO·RR에서 호출 차단, 기부받은 서버의 중간 작업 선행, 회신 후 호출자 재개, 기부 회수 후 중간 작업 진척을 관측한다.

수신 전·토큰 발급 후 만료, 타인·중복 회신, 슬롯 재사용 후 이전 토큰, 호출자 및 서버 종료를 각각 확인한다. 단일 종료 상태, 순위 복원, 자원 회수와 후속 정상 호출을 기록한다. 게스트에서 강제하기 어려운 같은 시각의 회신·만료, 순환 및 정리 경쟁은 독립 호스트 사례로 확인한다. VER-024에 실제 산출물·호출 상태·소유자 전환과 독립 재생 결과를 남긴 뒤 #24 체크리스트에 반영한다. [REQ-024](requirements.md) · [DES-013](architecture.md) · [VER-024](verification-plan.md)
