# Pi 4 지정 코어 깨우기 BSP 소스 규약

[English](pi4-smp-bsp.en.md) · [CHG-070 기준 계획](pi4-smp.md)

JNOX_BOARD=pi4, JNOX_PROFILE=smp, JNOX_APP=directed-wake를 선택하고 벤치마크 기능과 다른 작업 선택자는 비워 둔다. 원시 이미지 적재 주소는 0x80000이다. 기존 Pi 4 링커의 메모리 창은 NOLOAD 객체와 CPU0 부팅 스택을 포함해 0x100000 직전에 끝난다. 빌드를 동결하기 전에 함께 개발하는 pi4_smp_workload 모듈을 연결해 전체 ELF를 구성한다.

## 진입 및 메모리 매핑

CPU0만 BSS를 지운다. 두 코어 모두 주소 변환과 캐시가 꺼진 콜드 상태에서 물리 주소와 같은 주소로 진입해야 한다. 보조 코어 진입 코드는 전용 16 KiB 스택을 사용하기 전에 MPIDR의 모든 affinity 필드와 진입 EL을 검사한다. EL2 진입 경로는 EL1h로 전환하기 전에 비보안 EL1 물리 카운터와 타이머 접근을 허용한다. EL1 직접 진입 경로는 로더가 이미 해당 접근을 허용했어야 한다. CPU2와 CPU3은 로더의 대기 루프에 머문다.

두 코어는 같은 TTBR0·TCR·MAIR와 EL1 매핑 및 정렬된 VBAR를 사용한다. 예외 프레임은 각 코어에서 중단된 작업의 전용 스택에 저장한다. 이미지 창은 Normal inner-shareable WBWA 메모리로, GPIO·UART·GIC 페이지는 Device-nGnRnE로 매핑한다. 스핀 테이블은 실행 금지 속성의 별도 Normal 페이지에 있다. 낮은 주소의 인계 영역에서는 CPU1 슬롯 0xe0이 비었는지 확인하고 리틀엔디언 64비트 진입 주소를 한 번 쓴다. 그 캐시 라인을 일관성 지점(PoC)까지 정리한 뒤 DSB SY와 SEV를 수행한다. 물리 보드에서 쓰기 전 실제 DTB, 펌웨어, 보안 구성, 예약 메모리와 CPU1 스핀 테이블 주소를 확인한다. BSP는 Cortex-A72의 CPUECTLR.SMPEN을 되읽어 기록한다. GIC SecurityExtn이 있는 하드웨어에서는 이 비트가 설정돼야 한다. 고정 QEMU 모델에서는 읽으면 0이고 쓰기는 무시되는 RAZ/WI 동작이 가능하다. 스택·주소 변환 준비 전 실패하면 출력 없이 대기할 수 있다.

## 작업 인터페이스

부팅 상태, 코어별 GIC 레지스터와 VBAR를 설정한 뒤, BSP는 IRQ가 가려진 상태로 pi4_smp_workload::start(0 또는 1)에 진입한다. CPU0은 공유 작업 상태 초기화 뒤 release_cpu1을 한 번 호출한다. 작업 모듈은 경계 감시값이 있는 EL1 작업 스택과 프레임을 준비하고 context::sched_start_frame으로 시작한다. context.S는 BSP에서 한 번만 포함한다.

CPU1의 유효한 SGI에 대해서는 irq(core, outgoing_frame, full_iar)를 호출한다. 반환되면 원래 IAR 전체를 GICC_EOIR에 쓰고 배리어를 수행한 뒤 eoi_done(core, iar)를 호출하고 선택한 프레임을 어셈블리에 반환한다. EL1 AArch64 SVC에는 svc(core, outgoing_frame, immediate)를 호출한다. 잘못된 인터럽트 ID나 송신 코어의 원시 IAR는 failure_iar()로 확인한다. 예상과 다른 유효 인터럽트는 spurious ID를 제외하고 치명적 정지 전에 EOI를 수행한다. CPU0만 직렬 출력을 한다. 생산자가 계속 실행 중이면 시간 초과 시 failure()를 보고할 수 있다.

ticks()와 frequency()는 각각 아키텍처 카운터 표본과 주파수를 반환한다. identity()는 MPIDR와 CurrentEL의 원시 값(EL1은 4)을 반환한다. boot_state(cpu)의 여덟 필드는 MPIDR, 진입 시 CurrentEL, 현재 EL, TTBR0, TCR, MAIR, SCTLR, CPUECTLR 순이다. gic_state(cpu)는 TYPER, 해당 코어의 대상 마스크, IGROUPR0, 디스트리뷰터 CTLR, CPU 인터페이스 CTLR, PMR, SGI1 우선순위, ISENABLER0 순이다. 대상 마스크와 함께 두 배열을 게시하며, 0인 마스크는 아직 게시하지 않은 인터페이스를 뜻한다.

물리 보드의 비보안 인터페이스에서는 코어별 그룹 레지스터가 0으로 읽힐 수 있어 이 값 하나로 위임 여부를 판정할 수 없다. 모델의 결합 인터페이스에서는 Group 1을 명시적으로 설정하고 되읽는다. BSP는 결합 EOI 모드, 인터럽트 우선순위와 마스크, 각 코어의 서로 다른 단일 대상 비트를 검사한다. CPU1에서 수락하려면 SGI1 IAR의 송신 CPU 번호가 CPU0에서 확인한 대상 비트 위치와 일치해야 한다. send_sgi1()은 DSB ISHST, 디스트리뷰터 쓰기, 완료 배리어 뒤 실제 쓴 SGIR 값을 반환한다. 대상 필드는 CPU1에서 관측한 마스크이고 대상 필터는 0, SGI 번호는 1, NSATT는 1이다. 호출 전후 카운터 구간에는 두 배리어가 포함된다.

기존 Pi 4 선택자는 조건부 어셈블리로 기존 부팅 경로를 사용한다. 바이트 동일성은 기존 구성의 재빌드와 연결 이미지 대조로 검증한다. 펌웨어 기동, 두 코어 실행, SGI 전달, 올바른 복귀, 물리 보드 일관성과 타이밍, 작업 완료 판정에는 각각 별도 관측이 필요하다.
