# J-NOX

[English](README.md) · 한국어

J-NOX는 기능안전 요구사항이 있는 임베디드 시스템을 위해 Rust로 개발하는 실시간 운영체제입니다. 예측 가능한 실행 시간과 격리, 멀티스레딩, 응용 프로그램과 장치 드라이버 지원을 우선합니다.

J-NOX는 **Just-enough Nucleus, Open eXtensions**의 약자입니다. 설계의 중심에는 작은 신뢰 커널과 격리된 서비스가 있습니다. 네이티브 Rust 드라이버와 Linux 드라이버용 별도 호환 영역을 지원하는 것이 목표입니다.

AArch64 QEMU `virt`에서 네이티브 앱은 사용자 모드(EL0)에서 단조 시계의 경과 시간을 읽을 수 있습니다. C에서는 `clock_gettime(CLOCK_MONOTONIC, &ts)`를 호출하고 Rust에서는 안전한 `jnox_native::monotonic_time()`으로 성공 시 `Duration`을 받습니다. [응용 프로그램 시계 검증](docs/evidence/evd-112.md) 단조 시계의 절대 기한까지 기다리려면 C에서 `clock_nanosleep`, Rust에서 `jnox_native::sleep_until`을 호출합니다. [대기 예제와 실행 명령](docs/native-c-abi.md#단조-시계의-절대-기한까지-대기)

작업 스레드에 공유 표본 버퍼를 넘기고 조인으로 결과를 회수할 수 있습니다. 저장소 루트에서 협력형 C 예제를 빌드합니다.

~~~sh
python3 tools/build_native_el0.py --out output/native-threads/c --threads 2 --source examples/native-threads/sensor.c
~~~

[네이티브 작업 스레드 생성 및 조인](examples/native-threads/README.md)

같은 센서 실행 파일을 두 슬롯에 배치하고 각각 다른 인수를 전달할 수 있습니다. Linux 또는 WSL의 저장소 루트에서 아래 명령을 실행하면 주 앱은 [16,32)의 합을, 동료는 [64,96)의 최솟값과 최댓값을 계산합니다.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c sensor.elf
python3 tools/build_native_el0.py --out sensor-pair --elf sensor.elf \
    --arg sum --arg 16 --arg 32 --arg 'west rack' \
    --peer-elf sensor.elf --peer-arg minmax --peer-arg 64 --peer-arg 96 --peer-arg ''
~~~

[패키지 부팅과 Rust UTF-8 예제](examples/native-argv/README.md)

C·Rust 앱을 주기 또는 상대 마감 순위로 스케줄링해 반복 실행할 수 있습니다. 각 앱은 주기 사이에도 자기 상태를 유지합니다. [네이티브 주기 작업 패키징](docs/native-c-abi.md#네이티브-주기-작업의-rmdm-실행)

네이티브 작업 최대 세 개에 시작 가능 시각·마감·예상 CPU 비용을 지정할 수 있습니다. 격자 경계에서 긴급 작업이 실행 가능해지면 EDF가 배치 계산을 선점할 수 있습니다. [네이티브 EDF 패키지 실행](docs/native-c-abi.md#네이티브-앱의-격자-기반-edf)

Linux 또는 WSL의 저장소 루트에서 새 출력 디렉터리에 가중 분석 패키지를 빌드한 후 C CRC·Rust 히스토그램 쌍을 단일 QEMU CPU에서 실행한다.

~~~sh
python3 tools/weighted_analytics.py build --out output/weighted-analytics
python3 tools/weighted_analytics.py boot --out output/weighted-analytics --variant pair
~~~

[가중 분석 패키지 운영 안내](docs/weighted-analytics-deployment.en-ko.md)

네이티브 ELF마다 가중치를 지정해 C 체크섬 계산과 Rust 히스토그램 계산처럼 준비된 앱 간에 CPU 서비스를 배분할 수 있습니다. [가중 패키지 실행](docs/native-c-abi.md#네이티브-앱의-가중-cpu-서비스)

독립 빌드한 C·Rust 앱을 최대 세 개까지 패키징하고 각 앱에 고정 우선순위를 줄 수 있습니다. FIFO는 동순위 순서를 유지하고 RR은 명시적인 양보 없이 동순위 계산을 교대로 실행합니다. [고정 우선순위 앱 실행](docs/native-c-abi.md)

네이티브 C 프로세스와 별도로 빌드한 Rust 프로세스를 한 이미지에 담아 각자의 데이터를 유지하면서 CPU0을 서로 양보할 수 있습니다. [네이티브 프로세스 두 개 패키징 및 실행](docs/native-c-abi.md#협력형-네이티브-프로세스-두-개-실행)

AArch64 QEMU `virt`에서 네이티브 v1 SDK로 빌드한 정적 C 실행 파일 하나가 사용자 모드(EL0)에 진입해 37을 계산하고 종료값 37을 전달했습니다. 표준 출력 쓰기 시스템 호출은 `-ENOSYS`를 반환했고 출력은 0바이트였습니다. [네이티브 C 시작·종료 검증](docs/native-el0-startup.md)

AArch64 QEMU에서 커널에 링크한 Rust 작업을 고정 우선순위 FIFO와 라운드로빈으로 실행할 수 있습니다. 두 정책 모두 Raspberry Pi 4 모델에서도 부팅했습니다. 별도의 Pi 4 모델 실행 6회에서는 중단된 CRC 작업이 전역 스케줄링에서 다른 코어로, 분할·클러스터 스케줄링에서 같은 코어로 복귀해 계산을 마쳤습니다. [CRC 작업 복귀 기록](https://github.com/Romanof123/JNOX/blob/447e777d7a007fef405d5f9a6ac3c38ead475a15/docs/evidence/pi4-crc-mainline-first.en-ko.md) 스케줄링 출시 표에는 시험한 구성별 근거와 남은 작업이 연결되어 있습니다. [Rust 작업 실행](docs/fixed-runtime.md) · [Pi 4 모델 검증](docs/evidence/evd-031.md) · [스케줄링 출시 표](docs/scheduling-release-cases.md)

단일 코어 QEMU에서 세 배치 정책으로 표본 요약, 임계값 이상 표본 집계, 잘못된 입력 거절을 확인했습니다. 별도의 중첩 그룹 작업은 할당량 창 세 개에 걸쳐 재개되어 누적 합계 100224를 확인했습니다. [배치 결과](docs/evidence/admitted-telemetry-batch-first.en-ko.md) · [중첩 그룹 결과](docs/evidence/recurring-hierarchical-first.en-ko.md)

단일 코어 QEMU `virt` 모델에서는 마감이 가장 이른 작업을 선택하는 EDF, 일정 주기로 남은 여유 시간이 가장 짧은 작업을 선택하는 LLF, 가중치로 CPU 시간을 배분하는 가상 실행시간 스케줄러도 실행할 수 있습니다. EDF·LLF 시험은 선점된 Rust 작업이 저장된 명령 위치로 복귀하는지 확인합니다. 가중치 시험은 두 작업의 CPU 점유율과 잠든 동료를 깨운 뒤 실제로 실행되기까지 걸린 시간을 측정합니다. [EDF·LLF 실행](docs/ordinary-deadline-plan.md) · [가중치 작업 실행](docs/ordinary-weighted.md)

첫 기준선은 **기획 초안 0.1**입니다. 자동차 적용에서는 특정 제품 구성의 ISO 26262 ASIL D 안전 목표를 지원하고 ISO/SAE 21434에 따라 사이버보안을 개발하는 것을 목표로 합니다. 적용 제품 선정, 실보드 검증, 안전 상태 승인과 인증은 남아 있습니다. 가상 벤치에는 별도의 안전 매개변수를 정했습니다. [벤치 안전 결정](docs/safety-parameters.md)

| 시작 문서 | 다루는 내용 |
| --- | --- |
| [제품 기획](docs/product-plan.md) | 제품 목표, 지원 플랫폼, 범위, 단계별 산출물 |
| [플랫폼 기능 기준선](docs/platform-capabilities.md) | 기본 기능, 선택 서비스, 추가 기능 제안과 기능별 합격 조건 |
| [안전·보안 개념](docs/safety-security-concept.md) | 위험·위협 후보, 가정, 안전 경계와 초기 분석 계획 |
| [기능안전팀의 첫 벤치 결정](docs/safety-parameters.md) | 가상 출력의 안전 상태와 시간 예산, 제품 값 확정에 필요한 근거 |
| [시스템 요구사항](docs/requirements.md) | 추적 ID가 붙은 요구사항과 합격 기준 |
| [시스템 아키텍처](docs/architecture.md) | 커널, Linux 드라이버 도메인, 스케줄링, 포팅, 서비스 설계 |
| [커널 아키텍처 선택](arch/README.ko.md) | AArch64 백엔드와 빌드 인터페이스 |
| [애플리케이션 추적 계약](docs/application-tracing.md) | 사건과 요청 타임라인, 수집 한계 및 단계별 검증 제안 |
| [M1 Rust 커널 첫 부팅](docs/m1-boot.md) | AArch64 QEMU에서 부팅할 최소 커널의 범위와 UART 합격 조건 |
| [Pi 4 실보드 준비](docs/boards/pi4-p1.md) | SD 부팅 파일, 직렬 연결, 에뮬레이터와 실보드의 개별 인수 기준 |
| [M1 타이머 선점](docs/m1-scheduler.md) | 한 번의 부팅에서 타이머 인터럽트, 높은 우선순위 스레드 실행, 중단된 스레드 복귀를 확인하는 기준 |
| [M1 동일 우선순위 FIFO](docs/m1-fifo.md) | 같은 우선순위 스레드의 준비 순서, CPU 양보와 복귀를 확인하는 기준 |
| [Rust 작업 테이블로 커널 실행](docs/fixed-runtime.md) | AArch64 고정 우선순위 커널을 빌드하고 앱 작업 테이블을 선택해 FIFO·라운드로빈 실행 |
| [일반 Rust 배치 작업](docs/ordinary-batch.md) | 실행 중 작업 제출, 지연 도착, SJF·SRTF·HRRN 선택과 실제 프레임 검증 |
| [다단계 큐 실행](docs/ordinary-multilevel.md) | MLQ와 MLFQ에서 Rust 작업을 실행하고 대기, 우선순위 강등·상향이 실제 실행 순서에 미치는 영향 확인 |
| [스케줄링 정책과 라운드로빈](docs/scheduling-policies.md) | 고전·현대 정책의 선택·예산·격리 조건과 정책별 실행 검증 |
| [IPC 프로파일](docs/ipc-mechanisms.md) | 스케줄링 승인 후 개발할 IPC 10개 계열의 자원 한도와 인수 시험 |
| [검증 계획](docs/verification-plan.md) | 시험 방법, 합격 판정, 실기기 검증, 증거 형식 |
| [개발 수명주기와 변경관리](docs/lifecycle.md) | 기획에서 구현·검증까지의 문서 연결, 변경 시 재설계 및 재승인 절차 |
| [현재 작업과 팀 인계](https://github.com/Romanof123/JNOX/issues/6) | 진행 중인 티켓, 다음 담당 팀, 독립 검증 및 병합 보류 상태 |
| [추적 링크](trace/links.csv) | 문서 간 관계를 자동 검사할 수 있는 데이터 |

작업 양식은 [변경 요청](.github/ISSUE_TEMPLATE/change-request.md), [PR 인계](.github/pull_request_template.md), [검증 증거](docs/templates/verification-evidence.md)에서 확인할 수 있습니다.
