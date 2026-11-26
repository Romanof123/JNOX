# One-shot UART owner recovery / UART 드라이버 1회 복구

CHG-320 · REQ-015/016 · DES-009 · IMP-320 · VER-015/016 · EVD-320. [Driver lifecycle #5](https://github.com/Romanof123/JNOX/issues/5) · [Implementation and acceptance #415](https://github.com/Romanof123/JNOX/issues/415)

Package three separate address spaces in one single-core AArch64 virt FIFO image. Slot 0 owns the IRQ-driven PL011 display service. Slot 1 runs a periodic peer with no device rights. Slot 2 is a preloaded dormant replacement. Enable this profile by selecting display owner 0, UART RX interrupts and standby slot 2. All existing profiles retain their scheduling and device ownership rules.

On an explicit driver-reported failure (exit status 86) from the primary, revoke access, mask the peripheral and GIC line, clear pending state, clear the software FIFO and boundedly discard stale hardware input. Fail closed if a bounded drain cannot establish an empty device. Release the replacement only after successful quiescence. The successor starts with its own address space and reset counters and must explicitly initialize the device. Discard any input that arrived during the no-owner interval before granting access to new input. The peer never receives device rights. A normal primary exit dismisses the unused standby. A failed replacement exhausts the single permitted handoff. General synchronous exception recovery and DMA are separate work items.

Independently boot failure and normal-exit controls. Observe the baseline owner, a known stale payload, pending and masked interrupt state, a fresh successor, a new payload following initialization, both owner exits and the peer's uninterrupted sequence before and after the transfer. Preserve the actual serial bytes, CPU-side observations, independently read guest state, QMP measurements, package identities and terminal records.

단일 가상 CPU에서 주 드라이버, 권한 없는 주기 앱, 대기 중인 예비 드라이버를 서로 다른 주소 공간에 둔다. 주 드라이버가 오류 코드 86으로 종료하면 장치·인터럽트 권한과 대기 입력을 회수한다. 정리된 상태에서만 예비 드라이버를 시작한다. 예비 드라이버는 초기화 후 전송한 새 입력만 수신해야 한다. 정상 종료 시 예비 드라이버는 실행하지 않으며 한 번 복구한 뒤에는 또 다른 권한 승계를 허용하지 않는다. 실제 입력, 인터럽트 마스크, 앱의 연속 실행과 두 제어 부팅을 대조한다.

[Measured results and original evidence / 실측과 원본](evidence/native-uart-recovery.en-ko.md)
