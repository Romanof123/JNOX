# IPC-003 · FIFO pulses and latched event bits / FIFO 펄스와 이벤트 비트

CHG-304 defines local notifications for ordinary native processes on single-core AArch64 QEMU virt under FIFO or RR. Each `--notify-route SENDER:OWNER` grants pulse send and event set in that direction. Only the owner can receive. Synchronous call and mailbox grants confer their own distinct rights. This is a prospective contract under REQ-024, DES-013 and VER-024. [Decision and scope #369](https://github.com/Romanof123/JNOX/issues/369) · [Acceptance tracker #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-304는 단일 코어 AArch64 QEMU virt의 일반 FIFO·RR 프로세스 사이에서 로컬 알림을 전달하는 계약이다. 각 `--notify-route SENDER:OWNER`는 해당 방향으로 펄스를 보내고 이벤트 비트를 설정할 권한을 부여한다. 수신은 소유자만 할 수 있다. 동기 호출과 우편함의 권한은 각각 별도로 부여한다. 인수 판정은 #24에서 추적한다.

| native-v1 SVC 0x4a01 | Inputs / 입력 | Result / 결과 |
| --- | --- | --- |
| x8=26 pulse_send | x0 owner slot, x1 u32 code, x2 u64 word / 소유자 슬롯·코드·값 | x0 status / 상태 |
| x8=27 pulse_recv | x0 deadline / 수신 기한 | x0 status, x1 code and x2 word on success / 상태와 성공 시 코드·값 |
| x8=28 event_set | x0 owner slot, x1 nonzero u32 mask / 소유자 슬롯·0이 아닌 마스크 | x0 status / 상태 |
| x8=29 event_take | x0 nonzero u32 mask, x1 deadline / 0이 아닌 마스크·수신 기한 | x0 status, x1 matched bits on success / 상태와 성공 시 일치한 비트 |

Each owner has a four-record pulse FIFO and an event latch. Preserve each accepted pulse, including duplicate records, in arrival order. A fifth pulse to a full queue returns EAGAIN and preserves the four queued records. Event set ORs the supplied mask into the latch. Event take returns the intersection with the requested mask and clears only that intersection. Repeated sets of a pending bit coalesce. Check direction, mask and operand widths before mutation.

소유자마다 펄스 레코드 네 개를 저장하는 FIFO와 이벤트 비트 래치가 있다. 같은 펄스도 개별 레코드로 도착순을 유지한다. 큐가 가득 차면 다음 송신은 EAGAIN을 반환하고 원래 네 레코드를 보존한다. 이벤트 설정은 기존 래치에 마스크를 비트 OR한다. 수신은 요청 마스크와의 교집합만 반환하고 그 비트만 지운다. 수신 전에 같은 비트를 여러 번 설정해도 한 비트로 남는다. 변경 전에 방향 권한·마스크·피연산자 폭을 검사한다.

Serialize delivery, expiry and teardown in the masked trap/IRQ path. A process can block on one receive at a time. A matching arrival may complete that receive directly once. Zero deadline polls and returns EAGAIN if no pulse or requested bit is ready. UINT64_MAX waits indefinitely. A finite absolute monotonic deadline expires before delivery at equality. An expired receive leaves an arriving pulse or newly set bits for a subsequent receive. Committed notifications survive sender exit. Owner exit clears queued records, pending bits and its wait state.

인터럽트가 마스킹된 트랩·IRQ 경로에서 전달·만료·종료를 직렬화한다. 프로세스는 한 번에 수신 하나만 기다린다. 일치하는 알림이 도착하면 대기 중인 수신을 직접 한 번 완료할 수 있다. 기한 0은 즉시 확인하며 펄스나 요청 비트가 없으면 EAGAIN을 반환한다. UINT64_MAX는 무기한 대기다. 유한 단조 시계 절대 기한과 전달 시각이 같으면 만료가 먼저다. 만료된 수신은 이후 도착한 펄스나 설정된 비트를 소비하지 않는다. 송신자 종료 후에도 확정된 알림은 보존하고 소유자 종료 시 큐·래치·대기 상태를 정리한다.

## Native acceptance cases / 네이티브 인수 사례

Run ordinary C and Rust processes under FIFO and RR. Send two pulses and check both codes and values in order. Set the same event bit twice before receipt, take it once, then poll again. Fill the four-record FIFO, reject the next send and drain the original four in order. Set distinct bits, take a subset and recover the remainder. Exercise a waiting receiver, expiry immediately before delivery, unauthorized direction, out-of-width operands, sender exit and owner cleanup. Check that packages granting only call or mailbox rights refuse notification sends. Record actual return statuses, received values, peak FIFO occupancy, initial and final latch state, and post-teardown state. Physical and product safety approvals remain separate. [Original IPC cases](ipc-mechanisms.en.md)

일반 C·Rust 프로세스로 FIFO와 RR을 각각 실행한다. 펄스 두 개의 코드와 값이 순서대로 수신되는지 확인한다. 수신 전에 같은 이벤트 비트를 두 번 설정하고 한 번 소비한 뒤 다시 비차단 수신한다. FIFO 네 칸을 채운 뒤 추가 송신 거부와 원래 네 레코드의 순차 수신을 확인한다. 서로 다른 비트를 설정한 뒤 일부만 소비하고 나머지를 수신한다. 대기 수신자, 전달 직전 만료, 무권한 방향, 폭을 벗어난 인자, 송신자 종료와 소유자 종료 후 정리를 확인한다. 호출 또는 우편함 권한만 있는 패키지는 알림 송신을 거부해야 한다. 실제 반환 상태·수신값·최대 큐 점유량·래치 초기값과 최종값·종료 후 상태를 기록한다. 실물 및 제품 안전 승인은 별도로 추적한다. [원래 IPC 인수 사례](ipc-mechanisms.md)
