# Pi 4 P1 두 코어 고정 작업과 지정 깨우기

[English](pi4-smp.en.md) · [보드 #34](https://github.com/Romanof123/JNOX/issues/34) · [POL-013 #47](https://github.com/Romanof123/JNOX/issues/47)

CHG-070은 별도 선택 Pi 4 원시 이미지 계획이다. 기존 Pi 4 직렬 부팅·타이머·고정 FIFO/RR은 코어 0을 사용한다. EVD-017은 이미 QEMU virt 두 CPU의 배치·경합 이동을 다뤘다. 이번 범위는 보드별 보조 코어 기동과 실제 고정 작업 중 지정 코어 깨우기다. 기존 선택자·기능 우선순위·기준 이미지 바이트를 유지한다. 두 차례 콜드 모델 부팅 결과는 아래 증거에 기록했다. [보드](pi4-p1.md) · [기존 SMP](../m3-smp.md) · [고정 작업](pi4-fixed-runtime.md)

`JNOX_BOARD=pi4 JNOX_PROFILE=smp JNOX_APP=directed-wake`는 보조 코어 기동과 인터럽트 전달에 `pi4_smp_boot.S`와 `pi4_smp_bsp.rs`를, 고정 작업과 종료 시 UART 기록에 `pi4_smp_workload.rs`를 선택한다. 모델 콜드 부팅 2회의 결과는 아래에 기록했다. 실물 보드 검증은 대기 중이다. [BSP](pi4-smp-bsp.md) · [작업 및 UART 형식](pi4-smp-workload.md)

## 진입과 예약 영역

EVD-011 실행 파일 해시, 4코어·2 GiB raspi4b TCG와 -kernel kernel8.img를 사용한다. AArch64 원시 로더는 독립 실행 이미지에도 Linux식 부팅 경로를 택한다. PSCI 비활성 다중 CPU 조건에서 보조 코어는 0x300의 WFE/읽기/분기 루프로 리셋된다. MPIDR Aff0에 따른 리틀엔디언 64비트 슬롯은 0xd8, 0xe0, 0xe8, 0xf0이다. CPU1 슬롯에 한 번 쓰는 경우 외에는 [0,0x80000) 인계 영역을 보존한다. 펌웨어의 CPU3 슬롯은 0xf0/0xf4 매직·버전과 겹치며 0xf8/0xfc에는 32비트 DTB·커널 진입 주소가 있다. 이 영역·실제 DTB·기타 예약 메모리를 재사용하지 않는다. CPU0만 BSS를 초기화한다. 이미지·개별 정렬 스택과 보호값·페이지 테이블은 [0x80000,0x100000)에 배치하고 겹침·초과를 거부한다. [실행 파일 식별](../evidence/evd-011.json) · [모델 스텁](https://github.com/qemu/qemu/blob/005c61c0ef6f6ae615283440b4bb7be51b2c6ddd/hw/arm/raspi.c) · [로더 조건](https://github.com/qemu/qemu/blob/005c61c0ef6f6ae615283440b4bb7be51b2c6ddd/hw/arm/boot.c) · [링커](../../kernel/pi4.ld)

실물 Pi 4 Model B 1 GB는 고정 펌웨어, 하위 주변장치 주소, 기본 GIC 스텁을 사용한다. 공개 스텁은 예상 동작의 근거이며 실물 쓰기 전 실제 펌웨어·스텁과 DTB cpu-release-addr를 확인한다. 모델은 EEPROM·SD·VideoCore를 우회한다. PSCI나 구형 메일박스로 대체하지 않는다. CPU0 초기화 완료 후 정렬된 보조 진입 물리주소를 0xe0에 쓰고 캐시 사용 시 PoC까지 정리한 뒤 DSB SY, SEV를 실행한다. CPU1은 전체 MPIDR·진입 EL을 확인하고 Rust 호출 전 전용 스택을 선택한다. BSS를 지우지 않는다. 양쪽 EL1h·VBAR·초기 IRQ 마스크와 동일한 주소 변환·캐시 속성 및 필요한 EL2 타이머 권한을 확인한다. CPU2/3 슬롯은 쓰지 않는다. 런타임 원자 연산 전 일관성이 확보된 Normal inner-shareable 메모리와 GIC Device 매핑을 준비하고 캐시 별칭을 금지한다. [고정 펌웨어](https://github.com/raspberrypi/firmware/tree/bc7f439c234e19371115e07b57c366df59cc1bc7) · [스텁과 EL](https://github.com/raspberrypi/tools/blob/2e59fc67d465510179155973d2b959e50a440e47/armstubs/armstub8.S) · [BCM2711 설명서](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)

## 작업과 판정

1. CPU0 생산자와 CPU1 배경·수신자는 각기 고정된 EL1 Rust 프레임·보호 스택을 쓴다. 배경보다 높은 우선순위의 수신자는 세대 1, 2를 기다리며 차단된다. 폴링으로 준비 전환하지 않는다. 한 번에 한 거래만 허용한다. 세대 g의 생산자는 16(g−1)+1부터 연속 정수 16개의 합 136, 392를 계산한다. 수신자는 한 번씩 소비하고 두 배인 272, 784를 반환한다. 배경 Rust 루프는 독립 카운터 16회 증가마다 진행을 남긴다. 각 송신 전 새 진행과 처리·수신자 차단/종료 후 실제 추가 진행을 요구한다. 저널 용량과 중단 기한을 고정한다.
2. CPU1이 BLOCKED(g)를 공개하고 배경이 실행된 후 CPU0은 (g,합)을 release 공개하고 DSB ISHST 뒤 GICD_SGIR로 CPU1에만 Group 1 SGI 1을 한 번 보낸다. acquire로 응답을 확인한다. GICD=0xff841000, GICC=0xff842000이다. 분배기는 CPU0이 한 번 초기화하고 각 CPU는 IRQ 허용 전 뱅크별 그룹·우선순위·마스크·인터페이스·대상 ID를 확인한다. Group 1 접근/위임 및 대상 비트가 다르면 중단한다. CPU1은 GICC_IAR에서 SGI 1과 송신 CPU0을 확인하고 같은 세대 메시지를 acquire로 읽는다. BLOCKED→READY를 한 번만 원자 전환하고 배경 프레임을 저장하며 원래 IAR 전체를 EOI한다. 예외 복귀로 수신자를 실행하고 차단/종료 후 저장된 배경 지점으로 복귀한다. 그 후 다음 세대를 허용한다. [GIC 연결](https://github.com/qemu/qemu/blob/005c61c0ef6f6ae615283440b4bb7be51b2c6ddd/hw/arm/bcm2838.c)
3. 코어별 유한 저널에 커밋 순서와 손실 수를 둔다. CPU0 공개·실제 SGIR 쓰기, CPU1 IAR/송신자, 배경 ELR/SP/진행, 선택 프레임, 수신자 실측 진입·계속 PC/SP, 세대·결과, SVC, 재개 배경 PC/SP/진행을 세대·인과관계로 결합한다. 쓰기 명령의 전후 표본과 버스 도착은 별개다. 양쪽 MPIDR·CurrentEL·CNTFRQ·CNTPCT와 코어별 틱 차이를 기록한다. 공통 시점 또는 오프셋 보정 범위가 있을 때만 코어 간 경과를 제시한다. USB 도착은 전송 시간이다. IRQ/송신자 누락, 스택 보호값 오류, 중복 소유·세대, 대응 없는 점유, 기록 손실, 재개 진행 누락은 합계가 맞아도 실패다.
4. 첫 캡처 전 소스·실행 파일·ELF/역어셈블·원시 이미지·패키지·설정·관찰자 해시를 고정한다. 4코어 중 2개가 참여하는 모델의 첫 두 독립 부팅 모두에서 두 세대와 코어 식별 QEMU IRQ/SVC/ERET·링크 PC를 대조한다. 로그에 CPU가 없으면 독립 CPU 식별 캡처가 필요하다. 정지 후 CPU0만 출력한다. 호스트 30초 제한·종료 후 1초 정숙은 관찰 규칙이다. 모든 실패를 보존하고 호스트 재생은 CPU/송신자/세대 교환과 IRQ 누락을 거부해야 한다. 실물은 동일 보드·카드에서 입회 완전 전원 재인가 2회, 원시 직렬·소프트웨어 기록, 보드 리비전·EEPROM·펌웨어·설정·카드 재읽기 해시를 요구한다. 물리 명령 이력에는 외부 추적이 필요하다. 물리 시간·이동·공유 캐시 간섭·에너지·WCET·제품 할당은 미확정이다. [REQ-002/005/008/023](../requirements.md) · [DES-003/004/012](../architecture.md) · [VER-002/005/008/023](../verification-plan.md)

로컬 보드 소스 SHA-256(raspi.c / boot.c / bcm2838.c 순서): 35eb8dea9fdd5fa36d9e75c4a13f3a47906939d5759ed448dc1c09a2a03af6bc / 3ee33e386c78e816410d7f897d616a1f2c44203e7a609671184a95153412311f / a81db2b2fda5403e7a71daeb4760c3f2f003237cfadba3540d5e7ad375c58299. 캡처 때 실제 실행 파일 해시를 따로 기록한다.

첫 두 차례 콜드 부팅과 CPU별 예외·복귀 기록은 [독립 모델 검증 결과](../evidence/pi4-smp-first.md)에 있다.
