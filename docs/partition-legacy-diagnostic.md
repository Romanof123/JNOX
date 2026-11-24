# Legacy assembly partition first-failure diagnostic / 기존 어셈블리 파티션 최초 실패 진단

## Scope and acceptance / 범위와 수용 기준

CHG-050, IMP-047, EVD-047, [issue #110](https://github.com/Romanof123/JNOX/issues/110). Diagnostic design baseline a57a0cb; stacked source base 4d1eab9 ([#104](https://github.com/Romanof123/JNOX/pull/104)). Systems requires preservation of the ordinary legacy fixture's scheduling table, owner selection, IRQ handling, restore witnesses, counter reads and UART protocol. Verification requires a separately selectable simulator image to capture the first failed predicate, observed and expected operands, outgoing owner, active window and programmed target when known. Functional-safety scope is diagnostic observability. No guard may be bypassed, and a diagnostic result cannot establish why the earlier uninstrumented run emitted PART FAIL. Three independent captures are archived in [EVD-047](evidence/evd-047.en.md).

시스템 측 기준은 기존 실행의 시간표, 소유자 선택, IRQ 처리, 복귀 검증, 카운터 읽기와 UART 형식을 유지하는 것이다. 검증 측 기준은 별도 진단 이미지가 첫 실패 조건, 실제값과 기대값, 현재 작업, 활성 창 및 마지막으로 설정한 타이머 목표를 기록하는 것이다. 기능안전 범위는 실패 관측에 한정한다. 진단 실행은 기존 실패 기록의 원인을 소급 확정할 수 없다. 독립 실행 세 건은 [EVD-047](evidence/evd-047.md)에 보존했다.

## Selection / 선택

From kernel/, select exactly one Cargo feature on QEMU virt. Leave JNOX_PROFILE, JNOX_APP and unrelated feature selectors unset. Use separate output directories and retain ELF hash, compiler, QEMU argv/version, raw UART and first boot for each. The normal image still emits PART FAIL on a failed predicate. Diagnostic success retains the legacy PASS markers.

The existing backend order selects the benchmark first. When cooperative or another higher-ranked backend wins, its normal UART protocol applies and the partition diagnostic is inactive. With the partition backend selected, the standalone diagnostic and negative feature sets below retain their configuration checks. The ordinary P/Q backend selectors have their own validation.

기존 우선순위로 실행 백엔드를 먼저 선택한다. 협력형 등 우선순위가 높은 백엔드가 선택되면 파티션 진단은 비활성이고 선택된 백엔드의 기존 UART 형식이 적용된다. 파티션 백엔드가 선택된 경우에는 독립 진단과 음성 대조군에 대한 구성 검사가 유지된다.

| Image / 이미지 | Build command / 빌드 명령 |
|---|---|
| Normal assembly fixture / 기존 실행 | cargo build --locked --release --features partition-bench |
| First-failure diagnostic / 최초 실패 진단 | cargo build --locked --release --features partition-legacy-diag |
| Benign negative monitor / 진단 음성 대조군 | cargo build --locked --release --features partition-legacy-diag-negative |

The negative monitor checks an intentionally incorrect marker count at A's first marker (observed 0, expected 1), then stops. It exercises diagnostic output only. Real guard failures take precedence. The legacy comparator and IRQ-entry counter samples retain their original observation points; no extra counter read is needed for diagnostics. Diagnostic context stores and assembly branch layout can change execution timing. Separate the normal and diagnostic results.

일반 실행과 진단은 각각 별도 ELF로 만들고 해시와 옵션, 첫 부팅 UART를 보존한다. 음성 대조군은 A의 첫 마커에서 실제 마커 수 0을 기대값 1과 비교한 뒤 멈춘다. 먼저 실패한 기존 조건이 있으면 그 조건을 출력한다. 진단용 저장과 분기 배치는 실행 시점에 영향을 줄 수 있으므로 기존 실행의 타이밍 근거는 별도로 유지한다.

## First-error record / 최초 오류 기록

PART DIAG1 has nine 16-digit hexadecimal words followed by PART FAIL. In order they give guard number, observed operand, expected operand or inclusive lower limit, exclusive upper limit or contextual operand, selected worker, active window start, active window end, last programmed absolute comparator, and allocated partition. Worker 0/1/2 means A/B/idle. All ones means unavailable. Allocated partition all ones minus one means an unowned window. Window indices use the fixture quantum. Targets and timed operands use CNTPCT ticks. During a new selection, guard 12 carries the prospective target as its expected operand and the comparator field retains the last completed programming. Before first dispatch the worker field is the initial idle sentinel. Unclassified exceptions have no meaningful operand triple. Stop masks exceptions and emits at most one fixed-width record and the terminal failure line.

필드 순서는 조건 번호, 실제값, 기대값 또는 포함 하한, 미포함 상한 또는 부가값, 선택 작업, 창 시작, 창 끝, 마지막으로 프로그래밍한 절대 비교값, 창에 할당된 파티션이다. 작업 번호 0/1/2는 A/B/유휴를 뜻한다. 모든 비트가 1이면 미확정이며, 할당 파티션 값이 그보다 1 작으면 소유자 없는 창이다. 창 경계 단위는 논리 틱, 타이머 목표와 관측 시각 단위는 하드웨어 카운터 틱이다. 조건 12에서 기대값은 새 목표이며 마지막 비교값은 직전 설정값일 수 있다. 최초 디스패치 전에는 작업 필드가 초기 유휴 표식이다.

Guard numbers below are decimal. Ranges are half-open. / 아래 조건 번호는 십진수이며 범위의 상한은 포함하지 않는다.

| Guard | Predicate or operands / 조건 또는 피연산자 |
|---|---|
|1, 2, 3|Stack id ≤ idle; IRQ mask set; bench state exists / 스택 번호, IRQ 마스크, 상태 초기화|
|4, 5, 6|Boundary arithmetic (index, epoch, quantum); policy decision (tick, IRQ count, B ready); unsupported dispatch tick / 경계 계산, 정책 결정, 허용된 디스패치 틱|
|7, 8, 9|Allocated owner, selected runnable worker, next expected IRQ boundary / 할당 소유자, 선택 작업, 다음 경계|
|10, 11, 12|Selected frame non-null, saved resume PC matches, sampled dispatch counter < prospective target / 프레임, 복귀 PC, 디스패치 시각|
|13, 14, 15|Outgoing frame slot empty, interrupted SP exact, interrupted PC within task text / 기존 프레임 슬롯, 중단 SP·PC|
|16, 17, 18|Saved SPSR mode/mask, x19 witness, TPIDR_EL1 witness / 상태 레지스터와 두 식별값|
|19, 20, 21, 22|Progress witness upper bits; nonzero low count; count exceeds previous; stack witness / 진행 카운터와 스택 식별값|
|23, 24|Policy table constructor (partition count, major frame, window count); positive timer quantum / 시간표 구성과 타이머 단위|
|25, 26, 27|Marker owner, marker SP, expected marker order (IRQ count, prior marker count, packed id×2+resumed) / 마커 작업·SP·순서|
|28, 29, 30, 31|Interrupt ID, timer control status, expected IRQ tick, actual counter inside [boundary, next quantum) / IRQ 번호·제어 상태·예정 틱·실제 시각|
|32, 33, 34, 35|Final IRQ count, marker count, all frames present, all tasks have progress / 종료 시 IRQ·마커 수와 프레임·진행 상태|
|36, 37, 38|B-ready transition occurs under idle with two markers and B previously not ready / B 준비 전이 시 작업·마커·준비 상태|
|41, 42, 43, 44|Assembly restore progress delta ≤1, stack witness, thread-pointer witness, expected register witness / 어셈블리 복귀 시 진행량·스택·스레드 포인터·레지스터 검증|
|90, 91, 99|Unexpected vector, unexpected SVC, deliberate first-marker monitor negative / 기타 예외 벡터, 예기치 않은 SVC, 진단용 마커 대조군|

진단 설계 기준은 a57a0cb이며 통합 기준은 #104의 4d1eab9이다. 기존 최초 실패의 원인은 미확정이다. [진단 추적 #110](https://github.com/Romanof123/JNOX/issues/110)
