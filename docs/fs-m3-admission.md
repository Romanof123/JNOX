# FS-M3-01 conditional admission / 조건부 수용

CHG-044, #94. Scope is the initially idle, single-core virtual fixture in [the frozen decision](pol-004-m3-control-plan.md). RM and DM both rank control before sender. All quantities are integer microseconds. The counter must represent the targets exactly. Example inputs are hypothetical budgets.

For phase A or B, bound the whole active segment including work after its decision. From its absolute target allow ready jitter J, one lower-priority blocking bound B, and C including dispatch. IRQ interference counts all IRQ service that can intersect the subsequent busy window. Only admit if the whole segment finishes strictly before the next target. Starting idle, induction then excludes a previous control segment at either activation. The same worker must finish A by arming its absolute continuation and yielding atomically, resume once at B and finish its job there. Other sleeps, skips, early starts, unbounded retries and queued activations invalidate the model.

For a stream with nominal separation T, execution within [target,target+L], a window of length w can intersect at most ceil((w+L)/T) nominal arrivals (half-open execution intervals). Use each phase finish bound as L when analyzing sender. This deliberately loses phase correlation. IRQ streams require independent finite nominal-arrival-to-handler-finish bounds, including masking, pending and nested service. Timer IRQ must cover every 2500 us target, including sender release on coincident ticks. Other IRQs need their own rows. The number of sources and their costs must cover every runnable interference source.

Ready jitter is an elapsed target-to-ready prefix including the triggering IRQ through readiness. Only residual or newly arriving IRQ work executes after readiness. The envelope can pessimistically allow a whole IRQ overlapping that window. Blocking is residual lower-priority execution after readiness, excluding IRQ time. Assign worker dispatch/resume, timer programming, observer and traces to their executing context once. Include possible preemption overhead in the causing activation/IRQ. Demand C excludes waiting, jitter and blocking. No other work may run at equal/higher priority.

U bounds the error of one reported elapsed interval including both endpoints. Apply it once to each reported upper bound, using raw finish bounds for interference. Lower decision latency is zero by construction (no early execution); interval uncertainty cannot establish this property from an ambiguous trace. Bound gaps by 2500 plus the later segment finish plus U, including B to next A. Sender and last segment must finish by their deadlines with uncertainty. A missing bound or unsupported contract leaves admission unknown. Runtime evidence, input provenance, per-phase and final state observations and the separate ADR boundary case remain required.

한국어. 단일 코어 가상 벤치에서 두 정책 모두 제어 우선이다. 절대 목표부터 준비 지터, 준비 뒤의 잔여 블로킹, 양의 실행·디스패치 비용과 IRQ 간섭으로 각 위상 종료를 계산한다. 각 위상이 다음 목표 전에 끝나고 동일 작업이 절대 시각에 한 번만 재개되어야 한다. 송신 분석은 독립 위상 두 개의 종료 상한으로 이월 실행을 포함한다. IRQ별 최소 간격과 목표부터 처리 종료까지의 상한이 필요하다. 실행 문맥에 비용을 한 번만 배정하고 구간별 계측 오차를 한 번 더한다. 예시 수치는 가정이다. 실제 추적과 상한 근거, 보드 근거는 별도로 검증한다.

## Reproducible arithmetic / 재현 계산

Run from repository root using Python 3. The PowerShell commands create transient case files. In both policies the example phase bounds with uncertainty are 185 and 285 us. Sender is 695 us (40+310+20+110+210+5), job finish and gap both 2785 us. Each nominal 10 ms cycle releases 950 us of worker service including dispatch and four 20 us timer services. A sliding 10 ms window may also intersect carry-in; the response recurrence accounts for it. Raw bound and upper_with_uncertainty are relative to the respective nominal target.

~~~powershell
foreach ($p in 'rm','dm') { python tools/fs_m3_admission.py tools/fs_m3_example.json $p }
$case = Get-Content tools/fs_m3_example.json -Raw | ConvertFrom-Json
$path = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
function Check-Case {
    [IO.File]::WriteAllText($path, ($case | ConvertTo-Json -Depth 5))
    foreach ($p in 'rm','dm') { python tools/fs_m3_admission.py $path $p }
}
$case.b.service = 915
Check-Case # B 1000, inclusive; gap and job finish 3500
$case.b.service = 916
Check-Case # B 1001, refused
$case.b.service = 200
$case.irq_entry_lateness = 496
Check-Case # entry upper 501, refused independently
$case.b.PSObject.Properties.Remove('blocking')
Check-Case # unknown
Remove-Item -LiteralPath $path
~~~
## Host record and result semantics / 호스트 기록 및 결과 해석

CHG-045 / IMP-043 implements a conditional host analysis for POL-004 #94 under the CHG-044 fixture design. EVD-043 is reserved for independent host replay. Run `python tools/test_fs_m3_admission.py` to exercise the regression cases. Exit 0 (`conditional`) means all limits follow **if** the emitted assumptions and numeric bounds hold. Exit 1 (`refused`) means the supplied bounds did not prove every limit. The named unproved limits or recurrence witness identify the reason. Exit 2 (`unknown`) covers invalid, missing or unsupported input and inconclusive iteration. Invalid library inputs raise ValueError. No status reports an observed timing violation. Input intervals, including zeros, require external justification for the frozen implementation. The complete IRQ list accepts up to 64 additional sources.

POL-004 #94의 CHG-044 설계를 바탕으로 CHG-045 / IMP-043이 호스트 분석을 제공한다. EVD-043은 독립 호스트 재현을 위해 예약되었다. 위 명령으로 회귀 사례를 실행한다. 종료 코드 0은 출력된 가정과 입력 상한이 성립할 때 모든 한계를 증명했다는 뜻이다. 코드 1은 주어진 상한으로 일부 한계를 증명할 수 없다는 뜻이며 실제 위반 관측을 의미하지 않는다. 해당 항목이나 반복식 중간값을 출력한다. 코드 2는 누락·무효·미지원 입력 또는 계산 미결정이다. 라이브러리의 무효 입력은 ValueError를 발생시킨다. 추가 IRQ는 최대 64종이며 0을 포함한 모든 입력 상한은 동결된 구현에 대한 별도 근거가 필요하다.

단위는 정수 마이크로초이며 목표 시각을 카운터로 정확히 표현해야 한다. CPU가 유휴인 상태에서 시작한다. A가 절대 재개 시각을 설정하고 원자적으로 양보한 뒤 같은 작업이 B에서 한 번 재개되어 종료된다. 각 구간 종료가 다음 목표보다 빠름을 순서대로 증명하면 이전 활성화가 남지 않는다. 조기 실행·추가 대기·누락·무제한 재시도·대기열 누적이 있으면 이 가정은 성립하지 않는다.

명목 간격 T인 요청이 목표부터 L 이내에 실행을 모두 마치면 길이 w의 반열린 구간과 겹칠 수 있는 요청은 최대 ceil((w+L)/T)개이다. 각 위상의 실제 종료 상한을 L로 사용해 송신 간섭을 구한다. 위상 간 상관을 버리므로 보수적이다. IRQ 종료 상한에는 마스킹·중첩·보류 지연을 포함한다. 타이머 IRQ는 송신과 겹치는 경우를 포함해 2500 us마다 한 번이다. 다른 간섭원도 모두 명시한다.

지터 J는 목표부터 준비까지의 경과 시간이다. 준비 이후의 하위 우선순위 실행만 블로킹 B에 넣는다. IRQ 상한은 준비 이후와 겹치는 IRQ 한 건 전체를 보수적으로 허용한다. IRQ에서 수행한 비용을 작업의 CPU 수요로 중복 합산하지 않는다. 선점 부대 비용은 선점을 일으킨 소스에 배정하고 타이머 설정·관찰·추적 비용은 수행 문맥에 배정한다. 제어와 같거나 더 높은 우선순위의 미신고 작업은 허용되지 않는다.

U는 양 끝점을 포함한 경과 시간 오차이며 결과마다 한 번 적용한다. 간섭 계산에는 오차를 더하기 전 종료 상한을 쓴다. 판정이 조기 실행되지 않는 조건에서 간격 상한은 2500 + 다음 위상 종료 지연 + U이다. 이전 작업의 B부터 다음 작업의 A까지도 포함한다. 모호한 시각 기록으로 조기 실행 부재를 입증할 수 없다. 위 예시는 A/B 185/285 us, 송신 695 us, 작업 종료 및 최대 간격 2785 us이다. 명목 10 ms마다 작업 수요 950 us와 타이머 서비스 20 us 네 건이 발생한다. 임의의 10 ms 관찰 창에는 이전 요청의 실행도 포함될 수 있다. 실제 실행, 위상별 추적, 최종 출력과 ADR 단일 판정 경계 사례는 별도로 검증한다.
