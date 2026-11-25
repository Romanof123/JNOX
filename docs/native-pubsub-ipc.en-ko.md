# Native publish/subscribe service / 네이티브 발행·구독 서비스

CHG-309 follows REQ-024 → DES-013 → IPC-008 → VER-024. The planned local deployment uses four separately linked ordinary AArch64 EL0 processes on one QEMU virt CPU under fixed-priority FIFO and RR. One application publishes, two others subscribe and an isolated broker owns the topic and subscriber queues. IPC-008 remains open. The approved IPC family count under #24 is 5/10. [IPC-008 #384](https://github.com/Romanof123/JNOX/issues/384) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-309의 로컬 배포는 단일 코어 AArch64 QEMU virt에서 고정 우선순위 FIFO·RR로 각각 링크한 일반 EL0 프로세스 네 개를 실행한다. 발행자 하나, 구독자 둘, 토픽과 구독 큐를 소유하는 브로커 하나다. IPC-008은 검증 전이며 #24에서 승인된 계열은 5/10이다.

## Authority and lifetime / 권한과 수명

Bind independently declared publish and subscribe rights to the authenticated process and topic. Transport access to the broker conveys only the ability to submit an RPC. Check operation, caller, topic and generation before reading a queue or mutating state. Reject foreign handles and retire a subscription generation at unsubscribe or owner death. Re-subscription starts with an empty queue. At a full queue, discard the incoming sample for that subscriber and increment its loss counter once. Preserve already queued samples and continue delivering to other eligible subscribers.

발행·구독 권한을 각각 인증된 호출자와 토픽에 연결한다. 브로커 RPC 호출 권한만으로 발행·구독 권한을 얻지 않는다. 큐 조회·변경 전에 연산·호출자·토픽·세대를 확인한다. 다른 소유자의 핸들을 거부하고 구독 해지 또는 소유자 종료 시 세대를 폐기한다. 재구독은 빈 큐에서 시작한다. 한 구독자의 큐가 가득 차면 그 구독자에게 새로 들어온 샘플만 버리고 손실 수를 한 번 증가시킨다. 기존 큐의 순서와 다른 구독자의 전달을 유지한다.

## Bounded deployment / 배포 한도

Use an ordinary Rust broker in slot 3 and independently packaged C/Rust client ELFs in slots 0–2. Add a fourth ordinary fixed-profile slot for this deployment while preserving the existing three-slot ABI and scheduling trace. Provide a bounded public C/Rust SDK. The package binds separate publish and subscribe topic masks to each caller. Topic IDs are 0–3, with at most three live subscriptions per client. Each subscription has four records of at most 32 bytes each. Per-topic publication sequence, handle generation and per-subscription loss are 64-bit quantities. Reject overflow before any partial publication or handle reuse. Traverse live authorized subscriptions in registration order once per committed publication. Observation leaves the head and count intact. Client exit drops its subscriptions and queued data and wakes eligible blocked operations.

슬롯 3에는 일반 Rust 브로커, 슬롯 0~2에는 각각 패키징한 C·Rust 클라이언트를 둔다. 기존 3슬롯 ABI와 스케줄링 추적을 유지하며 이번 고정 우선순위 배포에 네 번째 슬롯을 추가한다. 유한한 공개 C·Rust SDK와 호출자별 발행·구독 토픽 마스크를 제공한다. 토픽 ID는 0~3, 클라이언트당 활성 구독은 최대 3개, 구독당 큐는 32바이트 이하 레코드 4개다. 토픽별 발행 순번·핸들 세대·구독별 손실 수는 64비트다. 오버플로는 일부 대상에만 전송하거나 핸들을 재사용하기 전에 거부한다. 발행 한 건마다 권한 있는 구독을 등록 순서로 한 번씩 처리한다. 조회는 큐의 선두와 수량을 유지한다. 종료한 클라이언트의 구독과 대기 데이터를 회수하고 조건을 충족한 대기를 깨운다.

## Required observations / 검증 사례

Run the four distinct ELFs on both schedulers. Record package and ELF hashes, slot assignments, independent rights, queue capacities, monotonically numbered input payloads, per-subscriber outputs and loss counters in private RAM, broker state and each process exit. Keep first failures and any missing trace rows. Separate host race/boundary checks from guest delivery evidence.

* Synchronize both subscriptions before the first publish. For a stream longer than queue capacity, let one subscriber drain each publication while the other stalls. The draining subscriber must receive every payload in order with zero losses. The stalled subscriber must receive precisely its original queued prefix and count every subsequent dropped new sample. For example, after six such publications the stalled queue must contain the first four records and report two losses. After draining it, publish a seventh and check recovery for both readers. Check empty queues and zero remaining subscriptions, records and pending waiters after client exit.
* Unsubscribe with unread data, re-subscribe, then try the retired handle and a foreign handle. Verify each refusal leaves the new subscription intact and that only post-subscription samples arrive. Exercise unauthorized publication and subscription independently; compare queue contents and counters before and after each refusal.
* Exercise a raw finite-deadline request that expires or is canceled before readiness. Capture the token and the broker's commit decision. Cause readiness from another process before the timed-out caller sends any follow-up RPC. Verify the canceled request caused no late delivery, consumed no eligible sample and changed no delivery or loss count. In the reverse order, verify one committed delivery with its matching token. A timeout status alone cannot establish either ordering.

두 스케줄러에서 서로 다른 ELF 네 개를 실행한다. 패키지·ELF 해시, 슬롯별 권한, 큐 용량, 발행 순번과 실제 페이로드, 각 구독자 사설 RAM의 수신값·손실 수, 브로커 상태와 프로세스별 종료값을 보존한다. 첫 실패와 추적 누락도 기록한다. 호스트 경계·경합 시험과 게스트 전달 관측은 구분한다.

* 첫 발행 전 두 구독을 완료한다. 큐 용량을 넘는 동안 한 구독자는 매번 읽고 다른 구독자는 멈춘다. 계속 읽은 쪽은 손실 없이 전부 순서대로 받아야 한다. 멈춘 쪽에는 최초 큐의 내용만 남고 이후 폐기된 신규 샘플 수만큼 손실이 증가해야 한다. 예를 들어 연속 6건 발행 후 멈춘 큐에는 처음 4건이 남고 손실은 2건이어야 한다. 큐를 비운 뒤 일곱 번째 샘플로 회복을 확인한다. 빈 큐 조회와 클라이언트 종료 후 잔여 구독·레코드·대기자 수 0을 확인한다.
* 미수신 데이터가 남은 상태에서 구독을 해지한 뒤 재구독한다. 이전 세대 핸들과 다른 소유자의 핸들을 거부하고 재구독 이후 데이터만 수신하는지 확인한다. 발행 금지와 구독 금지를 각각 시도해 큐와 카운터의 보존을 확인한다.
* 준비 전에 취소되거나 기한이 만료된 원시 RPC의 토큰과 브로커 반영 시점을 기록한다. 호출자가 후속 RPC를 보내기 전에 다른 프로세스에서 준비 조건을 만든다. 늦은 전달·샘플 소비·카운터 변경이 없는지 확인하고, 반대 순서에서는 일치하는 토큰으로 정확히 한 번만 전달되는지 확인한다.

This profile covers local IPC only. A remote bridge requires its own transport and evidence. / 이번 범위는 로컬 IPC다. 원격 브리지는 독립된 전송 프로필과 증거가 필요하다.
