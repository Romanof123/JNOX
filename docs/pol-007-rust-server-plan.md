# POL-007 ordinary Rust aperiodic servers / 일반 Rust 비주기 서버

[English](#english) · [한국어](#korean) · [#101](https://github.com/Romanof123/JNOX/issues/101) · CHG-047 · IMP-045 · EVD-045

<a id="english"></a>

## English

Freeze one Cortex-A72 QEMU virt/GICv2/TCG, 128 MiB. Run each pattern with polling (PS), deferrable (DS), and sporadic (SS with 2 pending credit entries and separately 1). Cold reset for each configuration. A slot is q=50 ms, F/20 counter ticks with F divisible by 1000. Use absolute E+tq and score slots 0–15; observe final close at 16. Each server has Q=2 indivisible units per P=4 slots. One successful service consumes one whole discrete entitlement even if the worker finishes early; the remaining time is idle. No rounding elapsed CPU into credits. Measure actual task and IRQ/kernel intervals separately. This is a finite slot-service benchmark. [Existing server policy](../kernel/src/aperiodic_policy.rs) · [EVD-016](evidence/evd-016.en.md)

Use ordinary Rust producer, server and periodic task contexts, each with a private EL1 stack. IRQ wakes the producer on declared arrivals. It submits actual FIFO request ID and units, then parks; only actual submissions by the boundary preparation cutoff qualify. Order within each slot: due releases submitted in order, periodic arbitration, server replenishment and decision, then execution. The higher-priority periodic task has T=4q, offset 2q, D=q, occupies its whole eligible slot for arbitration even if it finishes early. Its work per release is 256 wrapping u32 steps x=3x+1 from seed 1+release index; each server unit computes the same 256 steps from request ID plus zero-based unit index. Store observable per-step progress and final checksum. No second unit, reuse of slack or server service on periodic slots. Freeze counter target-to-IRQ ≤1 ms, slot target-to-completed dispatch preparation ≤5 ms, selected worker finish by the next boundary (periodic job measured from nominal release). Include uncertainty and cost of submission, arbitration, IRQ, instrumentation and switch. Before admission require documented bounds fitting each interval and the fixed 45 ms residual service allowance after maximum preparation; distinguish measured maxima, justified bounds and actual deadline verdicts.

| Pattern | Admitted FIFO arrivals (slot:units; consecutive IDs from 1) | Required actual server slots in order |
| --- | --- | --- |
| A, empty poll/straddling boundary | 1:1, 3:2, 4:1 | PS 4,5,8,9; DS 1,3,4,5; SS2 1,3,5,7; SS1 1,3,7,8 |
| B, backlog and renewal | 0:4, 4:2 | PS/DS/SS2 0,1,4,5,8,9; SS1 0,1,5,7,11,12 |

Admit these two finite manifests only. At most 4 outstanding FIFO IDs and 6 uncompleted units; request size 1–4. Before accepting check size, capacity, duplicate ID and arithmetic. Reject each invalid submit atomically, preserve previous FIFO/budget/credits. Exercise each limit with a separately initialized full fixture; do not inject extra work into positive cases. The final unit of each request must finish by the end of its last assigned slot in the row (FIFO allocation); record actual submit-to-finish time, not just the index. Periodic slots 2,6,10,14 must each complete independently. Verify requested=completed+pending at boundaries after reconciliation; at dispatch distinguish reserved/in-flight from retired work until physical completion. A missing, late or duplicated completion invalidates the run. Fail overload or stalled service visibly; never charge a fictitious completion.

PS replaces allocation at aligned boundaries and forfeits remaining quota on empty poll/drain; DS replaces, never adds, and retains unused quota until renewal. SS serves at t, schedules equal credit for t+4; replenish before dispatch even when preempted. At every step SS available+sum(pending credits)=2. On full credit storage merge only youngest with new debit at the later due time; no early reclaim, request loss or double credit. For A/SS1 the debit at 3 moves credit due 5 to 7 with amount 2. DS can legitimately serve either side of a boundary; check per-aligned-period cap and replacement, not a sliding-window cap. Monitor arrivals, completed FIFO unit identities and dated credits independently of kernel assertions.

Record CNTPCT/F for target, IRQ entry, actual producer submission/ack, dispatch, preemption, worker progress/result, finish/receipt and return. Preserve actual ELR/ERET, private stack ranges, ordered counter ties and first cold verdict per hashed ELF; correlate with an independent exception trace and offline state replay. Bound trace storage and delay UART until observation ends. Test equal/+1 tick timing verdicts, wrong owner or ledger, overflow, missing issue/return, and mid-service failure. Inject a finite IRQ mask spanning a boundary in a separate fault run, recording mask/unmask and its owner. At late IRQ report actual time and missed indices, retire only evidenced completions, abort qualification, freeze ledger and restart clean for a new trial. Never synthesize elapsed services or replenish a failed run by replaying missed dispatch. Whole-slot loss must fail even if later work finishes. If the image cannot expose a controlled delay, mark this live case pending and retain oracle-only controls separately.

REQ-002/005/023 → DES-003/012 → VER-002/005/023. The release matrix also requires actual M3 control interference. This periodic competitor is a synthetic probe; physical CPU, DMA, IRQ, board timing, product admission/WCET and the closed-loop control task remain open. [Release matrix](scheduling-release-cases.en.md) · [#21](https://github.com/Romanof123/JNOX/issues/21)

<a id="korean"></a>

## 한국어

단일 Cortex-A72 QEMU virt/GICv2/TCG, 128 MiB에서 위 두 패턴을 PS·DS·SS2·SS1별로 콜드 부팅한다. 카운터 주파수 F는 1000으로 나누어떨어져야 하며 q=F/20틱(50 ms), E 기준 절대 슬롯 0–15와 16의 종료 경계를 기록한다. 서버는 4슬롯마다 정수 단위 2개의 자격을 갖는다. 성공적인 단위 작업에 슬롯 자격 하나를 소비하고 조기 완료 후에는 유휴를 유지한다. 실측 CPU 시간을 올림·내림하여 예산으로 환산하지 않는다. 태스크·IRQ·커널 실행 시간은 별도 측정한다. [기존 정책](../kernel/src/aperiodic_policy.rs) · [EVD-016](evidence/evd-016.md)

전용 EL1 스택을 갖는 일반 Rust 생산자·서버·주기 작업을 사용한다. 도착 슬롯에서 IRQ로 생산자를 깨우고 실제 FIFO 요청 제출·대기 후 주기 작업 중재, 예산 보충·선택, 실행 순서로 진행한다. 주기 작업은 위상 2q, 주기 4q, 마감 q이며 슬롯 2·6·10·14를 우선 사용한다. 조기 종료해도 남은 슬롯은 유휴다. 각 주기 작업은 초깃값 1+릴리스 인덱스, 각 서버 단위는 요청 ID+0부터 시작하는 단위 인덱스로 u32 순환식 x=3x+1을 256회 수행해 단계별 진행과 결과를 남긴다. 슬롯당 한 단위만 실행한다. 목표부터 IRQ까지 1 ms 이하, 제출·중재·디스패치 준비 완료까지 5 ms 이하, 작업 종료는 다음 경계 이내로 정한다. 불확실성·계측·전환 비용을 포함한다. 주기 작업 마감은 명목 릴리스 기준이다. 수용 전에 각 구간 비용 및 준비 뒤 최소 45 ms의 실행 구간 적합성 근거를 제시하고 관측 최대·정당화한 상한·실제 마감 판정을 구분한다.

위 표의 도착만 수용한다. A는 슬롯 1·3·4에 1·2·1단위, B는 슬롯 0·4에 4·2단위이며 요청 ID는 1부터 순차 부여한다. 표의 서버별 실행 슬롯을 FIFO 순서로 배분하고 각 요청의 마지막 배정 슬롯 끝까지 실제 완료를 요구한다. 제출부터 종료까지 시간을 기록한다. 동시 미완료 ID 최대 4개, 미완료 단위 최대 6개, 요청당 1–4단위다. 크기·용량·중복 ID·산술 오류는 원자적으로 거부하고 기존 FIFO·예산·보충 내역을 보존한다. 한계별 별도 초기 상태에서 검사한다. 실제 완료 전에는 예약·진행 중인 단위를 완료와 구분하며 매 경계에서 제출 총량=완료+미완료를 대조한다. 누락·중복·지연·정체를 기록하고 무효 처리한다.

PS는 정렬 경계에서 할당을 교체하고 빈 폴링 또는 소진된 요청열에 남은 예산을 버린다. DS는 경계까지 예산을 보유하고 교체한다. 경계 양편의 연속 서비스가 가능하므로 정렬 구간별 상한을 확인한다. SS는 t의 서비스마다 t+4에 1단위를 돌려주고 선점 시에도 보충을 먼저 적용한다. 가용량+미래 보충량=2를 유지한다. 보충 저장소 포화 시 가장 늦은 항목만 새 차감과 합쳐 더 늦은 시각으로 미룬다. A/SS1의 슬롯 3 차감은 슬롯 5 예정분을 7로 미뤄 2단위로 만든다. 독립 판정기로 요청 손실·조기 보충·이중 보충을 확인한다.

동일 CNTPCT와 F로 목표·IRQ·실제 제출/승인·디스패치·선점·진행·결과·완료·복귀를 기록한다. ELF 해시별 첫 콜드 부팅 판정, ELR/ERET·스택 범위·동일 틱 순서, 외부 예외 추적과 독립 상태 재현을 보존한다. 캡처 용량을 미리 정하고 관측 종료 후 UART를 출력한다. 한계와 한 틱 초과, 잘못된 소유자·예산·오버플로·누락 사건·서비스 중 실패를 검사한다. 별도 결함 실행에서 경계에 걸쳐 IRQ를 유한 마스킹하고 소유자와 해제 시각을 남긴다. 늦은 IRQ에서는 실제 시각과 누락 슬롯을 기록하고 실제 완료만 정산한 뒤 판정 실패·원장 동결·새 시도 초기화를 수행한다. 밀린 가공 서비스나 보충을 재생하지 않는다. 통째로 잃은 슬롯은 이후 완료로 만회할 수 없다. 지연 주입을 지원하지 않는 이미지에서는 실제 사례를 미완료로 표시하고 판정기 단독 시험을 별도 기록한다.

REQ-002/005/023 → DES-003/012 → VER-002/005/023. 이 가상 주기 경쟁 작업은 M3 폐루프 제어 작업이 아니다. 제품별 수용·WCET, 실보드 CPU·DMA·IRQ 간섭 및 실제 M3 제어 작업 영향은 별도 근거가 필요하다. [출시 기준](scheduling-release-cases.md) · [#21](https://github.com/Romanof123/JNOX/issues/21)

First runtime verdict: PARTIAL. [EVD-045](evidence/evd-045.en.md) · 최초 런타임 판정은 부분 통과다. [한국어](evidence/evd-045.md)

## Prospective admission gate / 수용 판정 사전 계약

The existing FIFO and live receipt checks start a finite experiment without deciding whether a qualified cost budget exists. Keep that engineering entry point as the default. For any one of the eight normal applications select `JNOX_AP_MODE=admission` on the existing aperiodic profile. Before starting its timer or ledger, require a case-specific externally reviewed cost report. The integrator supplies its file (`JNOX_AP_REPORT`) and independently approved lowercase SHA-256 (`JNOX_AP_REPORT_SHA256`). Approval must cover this exact runtime build, model configuration, case, frequency, instrumentation and interference envelope. The pin identifies the approved bytes. Review and truth of the bounds are external responsibilities. No qualified report ships here. The deliberate late-IRQ arm remains engineering only.

기존 FIFO·실제 완료 검사는 유한 실험을 실행한다. 기본 실험 경로를 유지하고, 같은 비주기 프로필에서 위 8개 정상 앱 중 하나에 수용 모드를 명시한다. 타이머와 원장을 시작하기 전에 해당 앱의 외부 검토 보고서와 별도로 승인된 해시를 요구한다. 검토 범위는 정확한 빌드·가상 환경·앱·주파수·계측·간섭 조건이다. 해시는 보고서 식별자이며 상한의 타당성은 외부 검토 대상이다. 승인된 입력은 제공하지 않는다. 지연 IRQ 주입은 실험 전용이다.

Use UTF-8 report lines `key=value` in the following exact order. The target is `virt-a72-gicv2-tcg-1cpu-128m`, class is `justified-bound`, case is the literal app name and hz is actual CNTFRQ. All remaining fields are decimal nonnegative u64 **counter ticks per worst eligible slot**. Document every component (including any zero), path multiplicity, disjoint accounting, source/build identity and review in a nonempty rationale after the exact line `---`. A report with class `observed-maximum` or `synthetic-control` is ineligible. Reject missing, malformed or surplus header fields and overflowing paths.

보고서는 UTF-8이며 아래 순서의 필드를 사용한다. 단위는 해당 슬롯 최악 경로의 카운터 틱이다. 각 항목의 0값 근거, 반복 횟수, 중복 없는 경로 구분, 출처·빌드 식별·검토 내용을 구분선 다음에 기록한다. 관측 최댓값 및 합성 대조군은 수용 근거가 될 수 없다. 누락·잘못된 필드·산술 넘침은 거절한다.

```
target case class hz
irq_latency irq_interference irq_uncertainty
irq_handler submission arbitration preparation_switch preparation_instrumentation preparation_interference preparation_uncertainty
server_work periodic_work service_switch receipt service_instrumentation service_interference service_uncertainty
```

Add each IRQ component and require ≤1 ms. Add the entire IRQ sum and all preparation components and require ≤5 ms. Add max(server_work, periodic_work) and all common service components and require ≤45 ms. Interference terms cover all permitted blocking, preemption and IRQ demand in their respective segments. The switch/receipt terms must cover both ends of execution including actual receipt. Preserve actual IRQ/preparation/receipt deadline verdicts. A budget pass permits this configured trial only.

IRQ 합계 ≤1 ms, IRQ를 포함한 준비 합계 ≤5 ms, 서버와 주기 작업 중 큰 실행량에 공통 전환·실제 완료 접수·계측·간섭·불확실성을 더한 잔여 경로 ≤45 ms를 검사한다. 각 구간의 간섭에는 허용된 차단·선점·IRQ 총량을 포함한다. 수용 시에도 실행 중 실제 50 ms 경계 판정을 유지한다.

Acceptance for the scheduled slot: on host exercise all eight case bindings, exact limits and +1 tick per phase, larger periodic path, each absent component, overflow, wrong frequency/case/class, absent input and digest mismatch. On guest show admission denial before the first workload event with absent input, plus ordinary default behavior. A fabricated arithmetic positive is engineering evidence only. Real acceptance requires independent review of target-matched bounds and actual timely receipts for each applicable case. No board or product approval follows from these controls.

검증 슬롯에서는 호스트로 8개 앱별 결합, 각 경계와 1틱 초과, 더 큰 주기 작업 경로, 개별 필드 누락·넘침·주파수·앱·분류·해시 오류를 확인한다. 게스트에서 입력 부재 시 작업 시작 전 거부와 기존 기본 모드를 확인한다. 합성 양성은 계산 검사에 한한다. 실제 수용에는 대상에 맞는 상한의 독립 검토와 해당 앱별 실제 기한 내 접수가 필요하다.
