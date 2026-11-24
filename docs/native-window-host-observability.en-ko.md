# Prospective #216 systems decision / #216 시스템 후속 판별안

PM approved the zero-guest capability check under #216. The four diagnostic positions are conditional on the recorded capability decision. POL-006, REQ-002/005/023. This is a diagnostic feasibility plan. No new timing results.

## Existing observations / 기존 관측

[EVD-098 archive](evidence/evd-098.en.md) at b6d2da1, red-windows.csv, 14 generation-2 default-clock misses. Subtract opening from each archived sample. At positions 18, 29 and 32 selection already cost 68,928, 66,730 and 71,091 ticks. Even a free selection-to-user path leaves deficits of 6,428, 4,230 and 8,591 ticks. For the other eleven, pre-return was 57,103–61,396 and return-to-user 5,392–6,417 ticks. Their misses ranged from 88 to 4,626 ticks. Position 32 requires 14,522 ticks of total savings. A threshold crossing is cumulative. Retain original 62,500-tick opening-to-first-user rule and every prior outcome.

14개 미달 중 18·29·32번은 선택 시점에 이미 6,428·4,230·8,591틱 초과했다. 나머지 11개는 복귀 직전까지 한도 이내였으며 첫 사용자 관측까지 5,392~6,417틱이 더 걸렸다. 32번은 전체에서 14,522틱을 줄여야 한다. 누적 경계 통과만으로 원인을 배정할 수 없다.

## Narrow next decision / 다음 판별

Table P, ordinary scan/copy A, generation 2 versus later equivalent paths within the same boot. First perform a host-tool capability check on the actual archived QEMU version and launch environment, without new guests. Require a mapping of linked guest PCs to first generation timer entry, dispatch, return and idle user path, a host clock shared by translation events and vCPU scheduling intervals, and paired start/end timestamps for guest-block translation. Treat a count or an unaligned host stack sample as insufficient. Determine whether these events can be collected without rebuilding the production emulator. Record unavailable mappings before assigning any formal cohort.

P 원래 A 실행 파일에서 첫 타이머 처리·선택·복귀·유휴 사용자 코드의 실제 주소와 관측점을 먼저 연결한다. 사용했던 QEMU 및 호스트에서 블록 번역의 시작·끝과 vCPU 실행·대기 시간을 공통 시계로 대응할 수 있는지 게스트 실행 전에 확인한다. 대응이 불가능하면 새 정식 실행을 배정하지 않는다.

If supported, reserve one small diagnostic pilot in the serialized lane, unchanged guest ELF, same host/emulator configuration. Alternate two plain/traced pairs, default clock, P/A. Record first attempts, executable hashes, tracer overhead, and chronological nonoverlapping on-CPU translation, off-CPU and residual durations by mapped segment. Relate host durations to guest counter intervals only after verifying timebase alignment and its error. Mark a segment indeterminate when alignment/observer effects approach the deficit. Inspect later equal-path dispatches and record which guest PCs genuinely recur. The unit of evidence is a single traced run. Never subtract another boot's elapsed time to assign a cause. icount3 remains a separate functional/control clock in future qualification.

가능할 때만 P/A 기본 클록 평상·계측 2쌍을 교차해 진단한다. 첫 실행과 관찰 비용을 보존한다. 계측한 같은 실행에서 대응된 구간별 번역 중 실제 CPU 시간, vCPU 스케줄 대기 및 잔여 시간을 중복 없이 계산한다. 시계 대응 오차와 계측 영향이 초과분 수준이면 판별 보류한다. 초기 경로와 실제로 같은 후속 주소만 비교한다.

If translation consumes deficit-sized on-CPU time only on first-use mapped blocks, target those guest paths for a subsequent cold-path design. If off-CPU intervals alone are deficit-sized, investigate host scheduling controls before changing guest code. If both or neither account for the deficit, withhold attribution; remaining on-CPU time alone cannot identify a guest subroutine. Four diagnostic boots can discriminate an observed mechanism only in those traces. No qualification, promotion, host-wide bound or board inference follows.

최초 블록 번역이 해당 초과분 규모이면 그 코드 경로의 최초 비용을, vCPU 대기가 해당 규모이면 호스트 배정을 후속 대상으로 삼는다. 양쪽 모두 해당하거나 어느 쪽도 입증할 수 없으면 판정을 보류한다. 이 진단은 #233 프레임 보관, #249 인접 조회, #257 요인 실험과 별도이며 #232는 잠금 대기 작업이다. 기존 두 클록 인수 조건과 실기 검증을 유지한다.
