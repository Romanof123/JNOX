# IMP-054 virtual permit publication / 가상 허가 게시

[CHG-055 design and OFF characterization](ordinary-permit-phase-plan.md) · [Descriptor accounting](control-service-plan.md)

## Build and run / 빌드와 실행

From kernel/ on native Linux storage select virt, periodic, healthy, control-rm or control-dm, and JNOX_CONTROL_DESCRIPTOR=1. Unset marker, cold, fixed-trace and unrelated selectors. A0 omits both permit selectors. A1 adds JNOX_PERMIT_PHASE=nominal; A2 sets it to anchor. Operational images omit JNOX_PERMIT_PHASE_MODE. The separately identified OFF image requires JNOX_PERMIT_PHASE_MODE=characterize-off at build time. Use distinct native /tmp target directories for every policy, arm and mode. Freeze the source, compiler environment, ELF, clock, launch arguments, native paths, order and timeout before any trial.

```sh
export JNOX_BOARD=virt JNOX_PROFILE=periodic JNOX_APP=control-rm JNOX_CONTROL_CASE=healthy
export JNOX_CONTROL_DESCRIPTOR=1
unset JNOX_CONTROL_MARKERS JNOX_CONTROL_COLD JNOX_PERMIT_PHASE_MODE JNOX_PERMIT_PHASE
CARGO_TARGET_DIR=/tmp/jnox-a0-rm cargo build --release --locked
JNOX_PERMIT_PHASE=nominal CARGO_TARGET_DIR=/tmp/jnox-a1-rm cargo build --release --locked
JNOX_PERMIT_PHASE=anchor CARGO_TARGET_DIR=/tmp/jnox-a2-rm cargo build --release --locked
JNOX_PERMIT_PHASE=anchor JNOX_PERMIT_PHASE_MODE=characterize-off CARGO_TARGET_DIR=/tmp/jnox-char-a2-rm cargo build --release --locked
```

Repeat the separate characterization build with nominal for A1. Use the frozen QEMU virt/GICv2, one cortex-a72, TCG, 128 MiB, native storage, default affinity, first boot capture, and external exception tracing as assigned. Add `-icount shift=0,align=off,sleep=off` only for the predeclared deterministic positions. Name every file with mode/policy/clock/pair/ordinal. Anchor admission and characterization records to the same matrix position by manifest reference. Preserve all 16 default-clock and four deterministic admission comparison positions, including A0 provenance and A1/A2 refusal. Separately enumerate eight default-clock characterization positions (A1/A2 and A2/A1 per policy) and four deterministic (one A1/A2 pair per policy). An unrun or truncated position remains in its original denominator.

## Operation and measurement / 동작과 계측

Positive admission currently refuses before the operational timer or first worker is launched. Row 65 gives the missing-bit mask for frequency, evidence identifier, release-to-prepared, wake-to-worker, final poll, commit, restore/SVC, mask, receiver, baseline control, supervisor and IRQ propagation costs (bits 0–11 in order). No launch-time override exists. The native selector and evidence requirements for A0 remain independent.

In the compiled OFF entry the numeric values below constrain a finite measurement only. At the actual counter frequency, intervals convert microseconds downward to integer ticks. The ready, wake, final poll, commit, finish and mask cutoffs are respectively 250, 25, 50, 15, 50 and 25 µs. Retain measured costs and failures individually. None of these values establishes WCET. The common operational limits remain 750 µs total sender service, 10 ms response, 250 µs control service, 0.5 ms IRQ lateness, 1 ms decision phase, 5 ms maximum decision spacing and job deadline.

The sender privately prepares the payload. If needed it issues SVC 3, preserves its worker frame, waits with IRQs enabled on the processor and resumes that exact frame on one timer wake. Control release has priority on a coincident compare. The sender does bounded final polling, reads I, stores it in the prepared record, commits the sequence with a single STLR, executes DSB/ISB and reads U. A2 takes its generation anchor only from the first matching singleton publication; A1 continues to use the original nominal grid for targets. In both arms the live receiver acquire-loads the commit independently on an ordinary control activation and applies the existing expiry-first checks. The same supervisor paces the one explicit command after two accepted permits. The supervisor's ordered commit bracket defines command ingress; the separately measured controller acquire/consume can occur later. The command requires the entire bracket at anchor +15 ms. A measured issue sample alone proves no store instant.

Row 6 measures runnable slices. SVC 3 retains prior slices until the final row 8 debit. The sleep interval runs from SVC entry to actual wake/selection and counts toward elapsed response, while handler time is separately reported in 34/35. A second wake, missing original phase, budget violation, missing anchor, uncertain critical publication, or late preparation stops dependent work and retains unattempted indices. Normal deadline limits also apply if no wake is necessary. A freeze introducing a qualified cost record requires new review and independent accounting of compare programming, IRQ/SVC processing, receiver, mask, pre-ERET and actuator observer costs. The pre-ERET counter sample precedes final restoration instructions; external ERET must be corroborated against the exact ELF.

All application writes in the characterization binary pass through a compile-time hard-OFF PL061 interface. Record request, raw read before the write, write zero, barrier and raw read afterward for each ordinal. A detected ON or lost recording capacity immediately requests OFF, reads back, emits CONTROL CONTAINMENT FAIL with cause, detection/off-sample ticks and resulting bit, and dumps retained rows. A hypothetical accepted rearm emits row 84. The quiescent state is separately sampled at boot and termination, and idle reads the virtual device. This is coverage of the implemented QEMU peripheral write path and sampled readbacks. It does not establish continuous electrical observation or a real ON-to-OFF fault-response interval.

| Tag | Tick and two arguments |
|---|---|
|64/65/83|Arm (1 nominal, 2 anchor) and frequency; qualified admission and missing mask; OFF mode flag and evidence id|
|66/70/90|First rejected condition and sequence; inadmissible sender opportunity; last attempted and remaining opportunities|
|67/68/69|Wake compare and preexisting control target; requested wake and saved sender frame; SVC entry, requested wake and already-debited slices|
|71/72/73|Candidate target by sequence; remaining sender service at feasibility check; precommit polling observation by sequence and target|
|74/75/78|Publication I, U and sequence; qualifying first anchor; receiver lower/upper sample and commit sequence|
|76/77/79|Command I, U and intended target; distinct polling witness and generation; receiver acquire bracket and command generation|
|80/81/82/84/89|Write request ordinal/desired bit; corresponding before and after raw bits; hypothetical eligibility; terminal ordinal and raw bit|
|85/86/87/88|Stop CVAL/CTL, pending GIC word/DAIF, IRQ-entry CTL/IAR and CVAL/pending GIC word|
|91/92/93/94|Masked publication preparation to U; gate invalidation consumed/committed sequence; pre-ERET tick, frame and ELR; masked worker body lower/upper and worker id|

Rows 95 and 96 record any unfinished worker’s actual accrued service at stop, and the read-back sender wake comparator and control bits. Row 66 reason 16 indicates a wake-compare readback mismatch.

Row 66 reasons 1–15 identify owner/order, original grid/headroom, late resume/budget, invalid publication state, invalid bounds, uncertain store, incorrect gap, command prerequisites, command tick, ingress validity, command visibility, checked arithmetic, missed preparation, rejected delivery and missed sleep programming. Values 100 and above retain the original timing/expiry reason plus 100. A partial job at stop must be reported as partial. Row 94 excludes its instrumentation tail and the mask entry/exit instructions. Run `python3 tools/permit_phase_observations.py CAPTURE` for an observational summary, and `python3 tools/permit_phase_observations.py --synthetic-observer` for the standalone transient-ON coverage control. Independent QA retains the raw streams and separately checks scheduler and gap budgets, disassembly, barrier ordering and negative controls.

## 한국어

A0는 기존 기술자 실행이며 허가 위상 선택자를 설정하지 않는다. A1과 A2는 같은 독립 수신·명령·장치 경로를 사용한다. A1은 명목 목표를 유지하고 A2만 첫 실제 게시의 단일 틱을 기준으로 이후 목표를 잡는다. 일반 송신자는 같은 작업과 스택에서 한 번만 대기·재개할 수 있다. IRQ는 깨우기만 하며 허가를 게시하지 않는다. 커밋 전후 카운터 I·U와 소비 시각을 구분한다. 두 게시의 간격은 [I₂−U₁, U₂−I₁] 전체가 정확히 10 ms일 때만 확정한다. 감독자 명령도 실제 게시 구간 전체가 첫 기준 +15 ms여야 한다.

운영 상한의 근거가 비어 있으므로 현재 정상 수용은 작업 시작 전에 거부한다. 별도 characterize-off 실행 파일은 모든 장치 쓰기를 OFF로 강제하며 쓰기마다 요청값·직전값·직후값을 독립적으로 기록한다. ON 관측이나 추적 손실 시 즉시 OFF를 요청하고 관측 시각과 확인값을 보존한다. 80~82행과 마지막 89행을 대조하고 합성 일시적 ON 입력을 별도로 확인한다. 실제 물리 출력의 연속 관측이나 정상 재무장 근거로 사용할 수 없다.

기본 시계 16개와 결정적 시계 4개의 운영 비교 위치 및 거부를 보존한다. OFF 특성 측정은 별도의 기본 시계 8개와 결정적 시계 4개로 구분한다. 발행·재개·IRQ 마스킹·수신·복귀 비용과 조기 중단 후 남은 인덱스를 해당 실행에 귀속한다. 상한 미확정, 단일 틱 불확실성, 누락된 대기, 서비스 과다, 최초 실패를 그대로 보고한다. 최초 수용률의 분자에 특성 측정 결과를 넣지 않는다.

Hostile-input and one-tick-over controls still require independently retained replay or synthetic oracle fixtures with explicit provenance. The historical scripted boundary fixtures retain their own identity. This healthy-only source and the single transient-ON observer input do not establish those case verdicts.

[First virtual observations](evidence/evd-054.en.md) · [최초 가상 실행](evidence/evd-054.md)
