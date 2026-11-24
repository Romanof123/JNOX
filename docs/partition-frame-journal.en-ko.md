# Partition frame journal dispatch experiment / 파티션 프레임 기록 실험

Base f7d62aa. Preserve the first P/default generation 2 miss (A to idle while B is unready): deadline to IRQ 32630, IRQ to selection 34909, selection to restore 468, restore to first EL0 stamp 5385. Total 73392, limit 62500. Q/default and deterministic controls passed. This observation does not attribute cost to journal writes.

첫 P/default 실패와 62500틱 한도를 유지한다. B가 준비되지 않아 idle을 선택했다. 기존 관측만으로 기록 비용이 원인인지 알 수 없다.

Each frame has eight consecutive full-width rows. Reserve bounds once and publish the completed group once, retaining the same 104 words, tags, timestamps, order and row count. Incoming and outgoing observations stay in place. A halted writer can expose only previously committed groups via the cursor. IRQ-masked single writer and post-terminal replay are assumptions. No change to EL0 work, readiness, timer, MMU or policy checks.

프레임마다 연속된 여덟 행의 공간을 한 번 검사하고 모두 복사한 뒤 커서를 한 번 갱신한다. 중간 정지 시 커서는 마지막으로 완료한 그룹까지만 나타낸다. 두 EL0 작업, 전용 idle 및 모든 프레임 값과 측정 위치를 유지한다.

Prospective protocol: archive the historical four first runs individually. First run P/default, Q/default, P/deterministic and Q/deterministic on both revisions with identical build and capture settings per pair. For each cell perform four additional fresh-boot pairs in parent/candidate, candidate/parent, candidate/parent, parent/candidate order. Retain failures and incomplete captures. Replay both with the parent parser, compare decoded frame/row layout and transition invariants, and run existing weighted native compatibility cases. Report each generation's four stage deltas and total, especially P generation 2, as well as maximum and miss counts per boot. Compare matched pairs separately by cell. Keep the original bound even if a rerun passes. No execution evidence accompanies this source change.

과거 최초 네 결과를 각각 보존한다. 두 버전의 P/Q 및 기본/결정적 시계 네 조합에서 같은 설정으로 최초 실행하고 위 순서대로 교차 반복한다. 실패도 포함해 세대별 네 구간과 합계, 부팅별 최댓값과 실패 횟수를 보고한다. 기존 재생기와 weighted 호환 사례도 확인한다. 이 변경에는 실행 검증 결과가 없다.

[Independent paired first results and full frame replay / 독립 대조 결과와 전체 프레임 재생](evidence/native-partition-frame-pairs-first.en-ko.md).

[Independent decomposition of the preserved first IRQ intervals / 기존 최초 IRQ 시간의 독립 구간 분석](evidence/native-partition-first-irq-intervals.en-ko.md).
