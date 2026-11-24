# J-NOX

[English](README.md) · 한국어

J-NOX는 기능안전 요구사항이 있는 임베디드 시스템을 위해 Rust로 개발하는 경량 실시간 마이크로커널입니다.

J-NOX는 **Just-enough Nucleus, Open eXtensions**의 약자입니다. 서비스 격리, 네이티브 Rust 드라이버, Linux 드라이버를 위한 별도 호환 영역을 설계 목표로 삼습니다.

AArch64 QEMU에서 커널에 링크한 Rust 작업을 고정 우선순위 FIFO와 라운드로빈으로 실행할 수 있습니다. 두 정책 모두 Raspberry Pi 4 모델에서도 부팅했습니다. 스케줄링 출시 표에는 시험한 구성별 근거와 남은 작업이 연결되어 있습니다. [Rust 작업 실행](docs/fixed-runtime.md) · [Pi 4 모델 검증](docs/evidence/evd-031.md) · [스케줄링 출시 표](docs/scheduling-release-cases.md)

첫 기준선은 **기획 초안 0.1**입니다. 자동차 적용에서는 특정 제품 구성의 ISO 26262 ASIL D 안전 목표를 지원하고 ISO/SAE 21434에 따라 사이버보안을 개발하는 것을 목표로 합니다. 적용 제품 선정, 실보드 검증, 안전 상태 승인과 인증은 남아 있습니다. 가상 벤치에는 별도의 안전 매개변수를 정했습니다. [벤치 안전 결정](docs/safety-parameters.md)

| 시작 문서 | 다루는 내용 |
| --- | --- |
| [제품 기획](docs/product-plan.md) | 제품 목표, 지원 플랫폼, 범위, 단계별 산출물 |
| [플랫폼 기능 기준선](docs/platform-capabilities.md) | 기본 기능, 선택 서비스, 추가 기능 제안과 기능별 합격 조건 |
| [안전·보안 개념](docs/safety-security-concept.md) | 위험·위협 후보, 가정, 안전 경계와 초기 분석 계획 |
| [기능안전팀의 첫 벤치 결정](docs/safety-parameters.md) | 가상 출력의 안전 상태와 시간 예산, 제품 값 확정에 필요한 근거 |
| [시스템 요구사항](docs/requirements.md) | 추적 ID가 붙은 요구사항과 합격 기준 |
| [시스템 아키텍처](docs/architecture.md) | 커널, Linux 드라이버 도메인, 스케줄링, 포팅, 서비스 설계 |
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
| [추적 링크](trace/links.csv) | 문서 간 관계를 자동 검사할 수 있는 데이터 |

작업 양식은 [변경 요청](.github/ISSUE_TEMPLATE/change-request.md), [PR 인계](.github/pull_request_template.md), [검증 증거](docs/templates/verification-evidence.md)에서 확인할 수 있습니다.
