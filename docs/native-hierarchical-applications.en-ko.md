# Native hierarchical CPU service / 네이티브 앱의 계층형 CPU 서비스

[#333](https://github.com/Romanof123/JNOX/issues/333) · CHG-135 / IMP-135 / EVD-135 · [#21 POL-016](scheduling-release-cases.en.md). Prospective base `d13f8fd4538b28213d2e79ff69f86e02285df878`. The selected development deployment is AArch64 QEMU virt, Cortex-A72 TCG, one CPU, GICv2 PPI30, 128 MiB, default CNTFRQ 62,500,000 Hz. Ordinary SDK ELFs retain private process mappings. Systems owns these finite inputs, independent Verification scores the first records and PM records the scope in #21.

## Operator contract / 운영자 인터페이스

~~~sh
python3 tools/build_native_el0.py --out output/groups --policy hierarchy --hierarchy topology.json --elf checksum.elf --peer-elf histogram.elf --peer-elf energy.elf
~~~

The UTF-8 JSON object below is the full saturated configuration. Each child row is [parent ID, quota], each leaf row [child ID, quota]. Indices start at zero. Package slot i owns leaf i. Times are architectural counter ticks.

~~~json
{"period_ticks":15625000,"parents":[3906250,7812500],"children":[[0,15625000],[0,15625000],[1,15625000]],"leaves":[[0,15625000],[1,15625000],[2,15625000]]}
~~~

Accept 1–2 parents, 1–3 children, 1–3 leaves, exactly three charged levels and one leaf per ELF. Allow empty groups and several leaves per child. Require u64 integers, positive period and quotas ≤ period, and valid parent/child references. Reject wrong depth, missing/extra/duplicate keys, booleans, fractions, overflow, quota zero, counts above maximum, slot mismatch, missing/duplicate hierarchy option and conflicting policy/threads options. Reject invalid topology during packaging and direct build. Validate the embedded configuration again before operational timer arming or EL0 entry. Bind canonical topology, units, ordered ELF hashes, source and kernel identity in the ordinary manifest. Use the normal native virt launch.

운영자는 JSON 파일 한 개로 주기와 부모·자식·리프별 소속 및 예산을 틱으로 지정한다. 배열 순서가 ID며 패키지의 i번째 ELF는 i번째 리프다. 부모 1~2개, 자식 1~3개, 리프 1~3개와 세 단계만 수용한다. 빈 그룹과 한 자식의 여러 리프도 유효하다. 예산은 1~주기이고 상위 예산보다 커도 된다. 범위 밖 참조·개수, 타입·키·중복·충돌 옵션과 ELF 수 불일치를 패키지에서 거부한다. 직접 빌드 입력도 검사하며 커널은 첫 실행 전에 다시 확인한다.

## Execution and accounting / 실행과 회계

Reuse [Hierarchy](../kernel/src/hierarchy_policy.rs) and [#84 accounting](ordinary-groups.md). The adapter owns the immutable mapping, actual ready mask, absolute sleep deadlines, physical compare, idle and saved frames. Set the epoch after loading. Settle each open grant once before arbitration, apply due wakes and caller state transitions, then select by the existing hierarchy rank. Authorize sleep/yield/exit only for the actual issuer. If the common SVC code assumes the caller returns immediately, separate state mutation from return-frame selection. Restore the chosen owner's mapping, frame and saved continuation. Clock-driven waking uses the blocked process's own deadline. Nonblocking calls preserve entitlement. Retain early refunds, signed debt and fixed-grid refills, require positive credit on all three nodes, charge idle zero and program the earliest applicable grant end, wake or actionable refill. Terminate on all exit. Preserve linked callers and native defaults. No app symbols, exit values or progress addresses enter kernel decisions. The workload needs no new IPC.

네이티브 어댑터는 실제 호출자만 차단·양보·종료시키며 시각 도달로 해당 앱을 깨운다. 재선택 전에 기존 실행권을 한 번만 정산한다. 호출자와 복귀 대상이 다르면 실제 선택된 앱의 저장 프레임과 주소 공간을 복원한다. 기존 원장의 고정 위상 보충, 조기 반환 환급, 음수 부채와 세 단계 양수 자격을 유지한다. 유휴에는 부과하지 않는다. 실행권 만료·수면 종료·유휴 복구 중 가장 이른 시각을 사용한다. 커널은 앱 심벌이나 진행값을 인식할 필요가 없다.

## Frozen workloads and first positions / 고정 입력과 최초 실행

Build each [existing analytics ELF](weighted-analytics-deployment.en-ko.md#frozen-clients) once. CRC checks 524,288 byte ramps, 134,217,728 bytes, CRC 2,124,873,618, exit 237. Rust histogram checks 2,097,152 blocks, 536,870,912 samples, nine bins 2,097,152 × C(8,k), exit 70. Energy checks 4,194,304 signed ramps, 1,073,741,824 samples, sum of squares 5,864,240,971,776, exit 179. The timed reporter performs three absolute 40 ms waits and retains each requested and observed timestamp, final progress 3, mask 7, exit 7. Independently decode products from the owning private pages against the corresponding ELF.

| Position / 구성 | Slot order and changes to JSON / ELF 순서와 변경 | Required observation / 판정 |
|---|---|---|
| Saturated / 포화 | CRC, histogram, energy | In three distinct windows both ready siblings remain funded below exhausted parent 0 and neither dispatches before eligibility returns. Observe productive peer service, two strictly positive ordinary overruns at distinct timer events, three idle recoveries and all products. |
| Swap / 교환 | Energy, histogram, CRC, same quotas | At least one throttle/peer/refill sequence belongs to the exchanged ELF identities. Verify all products. |
| Child and leaf limits / 하위 예산 | CRC, histogram, energy. Set child 0 quota to floor(P/8) and leaf 1 quota to floor(P/16). | Child 0 depletes while parent and leaf 0 retain credit. Leaf 1 then depletes while its child and parent retain credit. Observe peer service and all products. |
| Wake / 깨우기 | Reporter, histogram, energy, baseline quotas | Three actual sleeps and due wakes, productive peer during sleep, caller's post-SVC frame restored and all private results. |

Capture first plain and externally traced cold boots of each position in table order. Preserve each failure and identify supplemental runs. Keep the linked #84 ordinary and #212 recurring plain/traced controls and a default native package control. Capture tool identities, argv, ELF hashes, UART, private RAM, correlated external exception/return PCs and raw rows. Stream or size records for gap-free sequence numbers from launch to exit for full-run cost and timing claims. Preserve every asserted throttle, refill, block/wake and exit. Missing/overwritten rows leave the affected claim unqualified. Verify positive progress from private data and final exits in plain and traced runs.

포화·교환·하위 예산·수면 구성 각각에 대해 일반과 외부 추적의 최초 콜드 부팅을 보존한다. 표의 실제 ELF 결과와 진행량, 부모·자식·리프 소진 순서, 다른 부모의 실행, 유휴 복구, 세 차례 수면의 실제 깨움과 복귀를 판정한다. 기존 연결형 일반·반복 작업과 기본 네이티브 패키지도 대조한다. 실패 원본과 보충 실행을 구분하고 원시 결정 순번·프레임·IRQ·앱별 RAM을 연결한다. 전체 실행 상한은 시작부터 최종 종료까지 누락 없이 관측된 경우에 평가한다.

## Isolation and costs / 격리와 비용

Apply [#212's timing criteria](nested-group-recurring-service.en-ko.md). P = 15,625,000 ticks. Ordinary and idle expiry require 0 ≤ actual IRQ sample − programmed compare ≤ P/8 = 1,953,125 ticks. Consecutive decision observations must be at most 8P = 125,000,000 ticks apart. Distinguish a sleep-driven early compare from the grant end. On uninterrupted quota-limited grants with compare equal to grant end, accepted occupancy is at most reserved grant + 1,953,125 ticks. For N such disjoint grants, sum of positive overruns ≤ N × 1,953,125 ticks. Report N and actual sum. Debt belongs to the outgoing path, and three accounting entries represent one physical interval. No whole-window Q+P/8 or individual peer deadline follows from this per-event bound.

Record sampled dispatch/settlement ticks, compare/IRQ sample, ready state, owners, all three balances/debts and frame addresses. Bracket measured handling with ordered architectural counter samples on exception entry, before/after settlement and arbitration, and after mapping/frame selection immediately before return. Report count, min/max, total ticks and maximum observed interval share for same owner, process switch, idle entry/recovery and SVC versus IRQ. Identify absent categories, trace mode and included probe overhead. Preserve the existing sample-to-sample attribution of switch overhead. Never debit diagnostic subintervals twice. The last pre-return sample precedes actual ERET. Independently replay early close, repeated debt across refills, stale observations, empty masks/groups, exact-zero recovery, long skipped periods and the u64 horizon using the pure ledger. Preserve original arithmetic checks. Use natural positive overshoot in the native cases and synthetic ticks only at the host boundary.

일반·유휴 만료는 IRQ 관측−비교 시각이 0~1,953,125틱, 연속 관측 간격은 125,000,000틱 이하여야 한다. 수면 때문에 앞당긴 비교값과 실행권 끝을 구분한다. 비교값이 예산 만료와 같은 연속 실행의 점유는 예약량+1,953,125틱 이하다. 해당 N회에서 양수 초과량 합은 N배 한도 안에 들어야 하며 실제 합도 남긴다. 세 노드에 기록된 동일 구간은 하나의 CPU 점유 구간이다. IRQ·SVC 진입, 정산·선택 전후, 매핑과 프레임 선택 후 복귀 직전의 카운터로 동일 앱·앱 전환·유휴 진입과 복귀 비용의 표본 수·최소·최대·합계와 최대 비율을 구한다. 계측 시간을 포함하며 실행자에게 이중 부과하지 않는다. 순수 정책에서 반복 지연·정확한 0 잔액·이전 시각·빈 그룹·긴 누락·u64 경계를 확인한다. 앱에서 IRQ 지연을 주입하지 않는다. 가상 비용은 해당 입력과 모드의 관측값이다. Item/load authority remains #1, physical timing #34, and safety timing #10. 제품 부하·실보드 시간·안전 시간의 승인 주체는 각각 #1·#34·#10이다.

## Single-leaf control / 단일 리프 대조

After the four positions above, retain first plain and externally traced cold boots of the same CPU-bound CRC ELF alone. Use the normal native package command below and the exact contents shown for single.json.

~~~sh
python3 tools/build_native_el0.py --out output/hierarchy-one --policy hierarchy --hierarchy single.json --elf checksum.elf
~~~

~~~json
{"period_ticks":15625000,"parents":[15625000],"children":[[0,15625000]],"leaves":[[0,3906250]]}
~~~

Require at least one actual PPI30 interruption of running EL0 before exit. Show the leaf exhausted while its ancestors retain credit, followed by idle with zero charge, a timer-driven eligible refill and restoration of the interrupted CRC PC, SP and private mapping. Decode the initial and restored user exception state as EL0t with PSTATE.I clear (SPSR_EL1 bit 7 at ERET). Correlate the saved frame and external exception return with private progress after resumption. Apply the same 0…1,953,125 tick reaction bound to each quota and idle timer expiry, the 125,000,000 tick observation-spacing bound, and the existing gap-free coverage rules. Check all 134,217,728 bytes processed, CRC 2,124,873,618 and exit 237. Let the normal hardware timer interrupt the unmodified computation. Preserve missing events or exceeded bounds in the first verdict.

기존 네 구성 뒤에 같은 CRC ELF 하나를 위 설정으로 일반·외부 추적 모드에서 각각 최초 부팅한다. 리프 예산은 주기의 1/4이며 부모와 자식의 예산은 각각 한 주기다. 실제 EL0 계산 중 PPI30이 적어도 한 번 진입하고, 상위 예산이 남은 채 리프가 소진된 뒤 유휴 무과금, 타이머 보충과 중단 지점 복귀가 이어져야 한다. 최초 및 복구 ERET에서 EL0t와 IRQ 허용 상태(I=0)를 확인하고 PC·SP·매핑과 복귀 후 진행을 대조한다. 실행 및 유휴 만료마다 지연 한도 1,953,125틱, 연속 관측 간격 125,000,000틱과 전체 순번 조건을 적용한다. 지연 주입 없이 최종 처리 바이트 수·CRC·종료값까지 확인한다.
