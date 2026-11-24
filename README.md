# J-NOX

한국어 · [English](README.en.md)

**Just-enough Nucleus, Open eXtensions.** 기능안전을 목표로 하는 경량 실시간 마이크로커널과, 서비스·드라이버를 독립 실행 영역에서 확장하는 운영체제 프로젝트.

첫 기준선은 **기획 초안 0.1**이다. 자동차 적용에서 ISO 26262 ASIL D 안전 목표를 지원할 수 있는 구성과 ISO/SAE 21434에 따른 사이버보안 개발을 목표로 한다. 첫 적용 시스템, 보드, 안전 상태 및 정량 시간 한계는 확정 전이며, 제품 적합성 평가와 인증은 아직 계획 단계다.

| 시작 문서 | 다루는 내용 |
| --- | --- |
| [제품 기획](docs/product-plan.md) | 제품 목표, 지원 플랫폼, 범위, 단계별 산출물 |
| [플랫폼 기능 기준선](docs/platform-capabilities.md) | 기본 기능, 선택 서비스, 추가 기능 제안과 기능별 합격 조건 |
| [안전·보안 개념](docs/safety-security-concept.md) | 위험·위협 후보, 가정, 안전 경계와 초기 분석 계획 |
| [기능안전팀의 첫 벤치 결정](docs/safety-parameters.md) | 가상 출력의 안전 상태와 시간 예산, 제품 값 확정에 필요한 근거 |
| [시스템 요구사항](docs/requirements.md) | 추적 ID가 붙은 요구사항과 합격 기준 |
| [시스템 아키텍처](docs/architecture.md) | 커널, Linux 드라이버 도메인, 스케줄링, 포팅, 서비스 설계 |
| [M1 Rust 커널 첫 부팅](docs/m1-boot.md) | AArch64 QEMU에서 부팅할 최소 커널의 범위와 UART 합격 조건 |
| [M1 타이머 선점](docs/m1-scheduler.md) | 한 번의 부팅에서 타이머 인터럽트, 높은 우선순위 스레드 실행, 중단된 스레드 복귀를 확인하는 기준 |
| [M1 동일 우선순위 FIFO](docs/m1-fifo.md) | 같은 우선순위 스레드의 준비 순서, CPU 양보와 복귀를 확인하는 기준 |
| [스케줄링 정책과 라운드로빈](docs/scheduling-policies.md) | 고전·현대 정책의 선택·예산·격리 조건과 정책별 실행 검증 |
| [검증 계획](docs/verification-plan.md) | 시험 방법, 합격 판정, 실기기 검증, 증거 형식 |
| [개발 수명주기와 변경관리](docs/lifecycle.md) | 기획에서 구현·검증까지의 문서 연결, 변경 시 재설계 및 재승인 절차 |
| [추적 링크](trace/links.csv) | 문서 간 관계를 자동 검사할 수 있는 데이터 |

작업 양식은 [변경 요청](.github/ISSUE_TEMPLATE/change-request.md), [PR 인계](.github/pull_request_template.md), [검증 증거](docs/templates/verification-evidence.md)에서 확인할 수 있습니다.
