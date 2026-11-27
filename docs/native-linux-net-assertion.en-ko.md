# CHG-335 · Device service assertion containment / 장치 서비스 단언 실패 격리

REQ-004 and REQ-011 cover source-driver execution and device authority. During the first #438 probe, an adapter assertion caused the native exception handler to retire the device and park the entire guest. This change routes an attributable AArch64 EL0 breakpoint through normal process termination on source-network IRQ profiles. Other exception classes, hardware faults, and protection of real-board DMA are outside this measurement. [Fault issue #440](https://github.com/Romanof123/JNOX/issues/440) · [Original lifecycle #438](https://github.com/Romanof123/JNOX/issues/438)

원본 네트워크 드라이버의 첫 probe 시험에서 호환 계층의 단언 실패가 전체 게스트를 정지시켰다. 이번 변경은 단일 CPU에서 현재 실행 프로세스가 발생시킨 AArch64 EL0 BRK의 처리와 장치 회수, 기존 소비자 앱의 생존을 다룬다. 다른 예외 종류, 실보드 DMA 격리 및 제품 안전 승인은 별도 검증 대상이다.

## Kernel behavior / 커널 동작

The masked EL1 handler checks the lower A64 synchronous vector, VBAR, saved execution mode, 32-bit BRK exception syndrome, active translation root/ASID and scheduler owner. It records the fault tick, PID, translation root, vector, ESR, ELR, SPSR, FAR, SP and LR in kernel memory before dispatching one Exit event. FAR is recorded as observed and has no address interpretation for BRK. Exit status is −5 and fixed-scheduler reason is 256 for this fault. Existing kernel invariant failures retain the platform stop path. [Exception entry and event dispatch](../kernel/src/native_el0.rs)

EL1에서 예외 벡터·실행 모드·예외 종류·페이지 테이블·현재 태스크를 대조하고 해당 슬롯의 레지스터와 시각을 기록한다. BRK의 FAR 값은 명령이 접근한 주소를 뜻하지 않는다. 기존 종료 경로에 한 번 진입해 장치 소유자의 IRQ와 DMA 권한을 회수하고 파이프 끝점을 닫은 뒤 남은 태스크를 선택한다. 리셋 확인 실패 시 기존 큐 격리 규칙을 적용한다. [Checked device reset](../kernel/src/native_linux_net_queues.rs)

## Directed guest and acceptance / 지정 게스트 및 합격 기준

Build the unchanged upstream driver with the lifecycle import and `--lifecycle-assert-test`, then package the two EL0 binaries under `--linux-source-net-lifecycle`. Inject one externally chosen frame. Capture the actual token, live RX grants, available and used indices, plus the peer's private frame before the driver executes its deliberate BRK. Join the protected exception PC and immediate to the linked instruction. Verify device status and queue reset readbacks, retired protected grants, pipe EOF, denied peer device calls, advancing peer timer and distinct signed process exits. Replay from an independent extraction and preserve the unmodified successful lifecycle regression. [Driver and peer](../examples/linux-source-net-lifecycle) · [Trace graph](../trace/links.csv)

원본 드라이버로 외부 프레임을 한 번 수신하고 소비자가 바이트를 확인한 뒤, 드라이버만 단언 명령을 실행한다. 발생 직전 장치가 소유한 실제 수신 버퍼, 명령 주소와 예외 레지스터, 발생 이후 큐와 권한 회수, 파이프 EOF 및 소비자의 타이머 진행을 연결한다. 최초 구현 시점에는 이 지정 시험의 게스트 증거와 독립 판정이 대기 중이다.
