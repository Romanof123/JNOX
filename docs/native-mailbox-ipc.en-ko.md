# IPC-002 · Native asynchronous mailbox / 네이티브 비동기 우편함

CHG-303 specifies IPC-002 for ordinary native processes on single-core AArch64 QEMU virt under FIFO or RR. Repeat `--mailbox-route CALLER:OWNER` to authorize directed sends between zero-based process slots. Each process receives only from its own mailbox. Packages may enable mailbox routes, synchronous `--ipc-route` calls, or both. Mailbox sends use no reply token. At #24 opening, 0/10 families had been accepted. Independent Release subsequently approved this two-app virtual FIFO/RR mailbox, the second of ten. [Release decision](evidence/native-mailbox-ipc-release.en-ko.md). #21, physical measurements and product approvals retain their existing scope. [Mailbox issue #367](https://github.com/Romanof123/JNOX/issues/367) · [Owner decision #365](https://github.com/Romanof123/JNOX/issues/365) · [Original acceptance #24](https://github.com/Romanof123/JNOX/issues/24) · [Synchronous contract](native-sync-ipc.en-ko.md)

CHG-303은 단일 코어 AArch64 QEMU virt의 일반 FIFO·RR 프로세스용 IPC-002 계약이다. 0부터 시작하는 프로세스 슬롯에 대해 `--mailbox-route CALLER:OWNER`를 반복 지정하여 송신 방향을 허가한다. 수신은 자기 우편함에서만 가능하다. 패키지는 우편함과 동기 `--ipc-route` 호출을 각각 또는 함께 사용할 수 있다. 우편함 송신에는 회신 토큰이 없다. 착수 당시 #24의 승인 수는 0/10이었다. 독립 배포팀은 이후 이 두 앱의 가상 FIFO·RR 우편함을 열 계열 중 두 번째로 승인했다. [출시 판정](evidence/native-mailbox-ipc-release.en-ko.md) #21 및 실물 계측·제품 승인은 기존 범위에서 계속한다.

| native-v1 SVC 0x4a01 | Input / 입력 | Result / 결과 |
| --- | --- | --- |
| SEND, x8=24 | x0 owner slot, x1 word, x2 deadline / 소유자 슬롯·메시지·기한 | x0 status / 상태 |
| RECEIVE, x8=25 | x0 deadline for the current process's mailbox / 자기 우편함 수신 기한 | x0 status, x1 word on success / 상태와 성공 시 메시지 |

Each owner has four slots of one u64 each. A successful send commits the complete word to endpoint storage or directly into a waiting receiver's saved result. The sender can then continue. Receive in FIFO commit order. When a send finds the mailbox full or a receive finds it empty, deadline zero returns EAGAIN. A future absolute monotonic deadline in nanoseconds allows blocking; UINT64_MAX permits waiting until the condition holds or the endpoint terminates. Each blocked sender retains at most one pending word. At a finite deadline, timeout wins over promotion at the same instant. Return ETIMEDOUT once and discard the uncommitted send.

소유자마다 u64 한 워드를 담는 슬롯 네 개를 둔다. 송신은 메시지 전체를 우편함에 복사하거나 대기 수신자의 저장된 결과에 직접 전달하면 완료된다. 송신자는 이후 진행하며 수신 순서는 전달 확정 순서(FIFO)다. 가득 찬 우편함에 송신하거나 빈 우편함에서 수신할 때 기한 0은 EAGAIN을 반환한다. 미래의 단조 시계 절대 기한(나노초)까지 대기할 수 있고 UINT64_MAX이면 조건 충족 또는 엔드포인트 종료까지 기다린다. 대기 송신자마다 미확정 워드는 최대 하나다. 슬롯이 비는 시각과 유한 기한이 같으면 만료가 우선한다. ETIMEDOUT을 한 번 반환하고 미확정 워드를 폐기한다.

Owner exit cancels its waiters and clears its queue. Sender exit discards its uncommitted word. Words already committed survive sender exit until consumed or the endpoint closes. Reject unauthorized directions before changing state. Serialize delivery, expiry and exit with scheduler wakeup so each blocked operation completes once and resumes through its saved frame.

소유자가 종료되면 해당 대기를 취소하고 큐를 비운다. 송신자가 종료되면 미확정 워드는 폐기하고 이미 확정된 메시지는 수신 또는 엔드포인트 종료까지 보존한다. 허가되지 않은 방향의 송신은 상태 변경 전에 거부한다. 전달·만료·종료와 스케줄러 깨우기를 직렬화해 차단된 연산이 한 번만 완료되고 저장된 프레임에서 재개되게 한다.

## First application cases / 최초 앱 사례

Run ordinary sender and receiver ELFs under both FIFO and RR. Send four distinguishable words, observe the full-queue immediate refusal, then let a fifth send expire before the receiver drains. Check that exactly the original four arrive once, in order. In a separate case free a slot before expiry and observe promotion, sender resumption and delivery. Compare expiry before, at and after the slot becomes available. Exercise direct delivery to an empty waiting receiver, empty nonblocking receive, unauthorized sends, sender exit before and after commit, and owner exit with waiters. Record actual statuses, words, peak occupancy and reclaimed queue/waiter state. Distinguish deterministic boundary checks from guest observations. Record the #24 verdict after independent replay. [REQ-024](requirements.en.md) · [DES-013](architecture.en.md) · [VER-024](verification-plan.en.md)

일반 송수신 ELF를 FIFO와 RR에서 각각 실행한다. 구별되는 네 워드를 채우고 즉시 송신 거부를 확인한 뒤, 수신자가 큐를 비우기 전에 다섯 번째 송신을 만료시킨다. 최초 네 메시지만 한 번씩 순서대로 도착하는지 확인한다. 별도 사례에서 기한 전에 슬롯을 비워 대기 송신의 확정·재개·전달을 확인한다. 슬롯 반환 전·동시·이후의 기한 경계를 구분한다. 빈 큐의 대기 수신자에게 직접 전달, 비차단 빈 수신, 무권한 송신, 확정 전후의 송신자 종료 및 대기자가 있는 소유자 종료를 확인한다. 실제 상태값·메시지·최대 점유량과 종료 후 큐·대기 자원 회수 결과를 기록한다. 결정론적 경계 검사와 게스트 관측을 구분하고 독립 재생 후 #24 판정을 기록한다. [REQ-024](requirements.md) · [DES-013](architecture.md) · [VER-024](verification-plan.md)
