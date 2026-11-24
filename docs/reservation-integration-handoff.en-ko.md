# POL-008 integrated ordinary first boots / 통합 일반 최초 부팅

Frozen integration protocol. Freeze four size-optimized Rust 1.95.0 release ELFs on the integrated tree using the existing virt reservation selector. Leave the stage selector unset. Each pair below uses one unchanged ELF. In order, run each listed position once in a new QEMU process. Only the external exception log option changes between modes.

동결된 통합 검증 절차다. 통합 트리에서 기존 virt 예약 선택자로 Rust 1.95.0 크기 최적화 릴리스 ELF 네 개를 고정한다. 단계 관측 선택자는 설정하지 않는다. 아래 순서대로 위치마다 새 QEMU 프로세스를 한 번 실행한다. 같은 앱의 두 위치는 동일한 ELF를 사용하며 외부 예외 로그 옵션만 바뀐다.

| Position / 순번 | JNOX_APP | Host observation / 호스트 관측 |
|---|---|---|
| 1 | aligned-wake | plain |
| 2 | cbs-wake | exception trace |
| 3 | aligned-saturated | exception trace |
| 4 | cbs-saturated | plain |
| 5 | aligned-wake | exception trace |
| 6 | cbs-wake | plain |
| 7 | aligned-saturated | plain |
| 8 | cbs-saturated | exception trace |

Use the original one-core Cortex-A72 virt/GICv2 default clock, native Linux temporary storage and 15 s limit. Freeze the image digest, command and position before launch. Preserve UART, stderr, host termination and any IRQ/SVC/ERET log from that process. Replay the ordinary ledger, work, return and allocation checks through the 34-slot wake or 10-slot saturated horizon. Retain every early stop and its raw guard operands. Apply the original 1 ms opening and cutoff-lateness limits, nominal end and allocation bounds. All eight must qualify for the eight-case gate. Count these eight separately from the 24 matched parent/candidate B/E/F launches and six additional controls in CHG-081.

기존 단일 Cortex-A72 virt/GICv2 기본 시계, Linux 네이티브 임시 저장소, 15초 제한을 사용한다. 부팅 전에 이미지 해시·명령·순번을 고정한다. 해당 프로세스의 UART, 표준 오류, 종료 상태와 해당 시 IRQ/SVC/ERET 로그를 보존한다. 깨움 34슬롯 또는 포화 10슬롯까지 일반 원장·작업·복귀·할당을 대조한다. 조기 중단은 원래 가드 피연산자와 함께 남긴다. 기존 개시 및 차단 지연 각각 1 ms, 명목 종료와 할당 한도를 그대로 판정한다. 8개 모두 통과해야 전체 일반 구성이 검증된다. 이번 8회는 CHG-081 부모·후보 B/E/F 24회 및 추가 대조 6회와 따로 집계한다.

[Ordinary build, capture and scoring / 빌드·수집·판정](pol-008-rust-reservations.md) · [Independent ledger and capture archive / 독립 원장과 자료](evidence/evd-053-v3.en.md) · [CHG-081 first observations / 최초 관측](evidence/reservation-chg081-first.en-ko.md)

[Integrated results](evidence/reservation-integrated-first.en.md) · [통합 결과](evidence/reservation-integrated-first.md)
