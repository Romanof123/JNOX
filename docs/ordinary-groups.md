# Ordinary nested group service · POL-016

## English

#84 under #21. CHG-040 / IMP-038. EVD-038 planned. Single-core virt, trusted linked EL1 Rust tasks, physical counter ticks, CNTP absolute compare and GICv2 PPI30. The supported maximum topology has two parents, three child groups and three leaves, with exactly three budgeted levels and one common fixed-grid period P. Each node has a positive quota Q ≤ P. Lower sibling index wins. block suspends the caller. wake(id) immediately readies a blocked leaf, may preempt the caller, and returns false for other states or invalid ids. Returning exits permanently. A system with no ready leaves runs WFI with timer disabled.

At each SVC or timer sample, charge the entire interval since dispatch once to the outgoing leaf, child and parent. Refund unused reservation immediately. Physical elapsed accounting includes switch overhead between samples. The IRQ compare is the scheduled end. Actual occupancy can exceed it until delivery. A late task incurs signed debt across grid boundaries. Each refill adds the quota and discards credit exceeding the quota. Every node on a path needs strictly positive credit before dispatch. The peer and idle incur zero charges for the late owner's service.

Settle through the first boundary, cap the refill there, subtract P−Q per complete busy window, then subtract the final prefix. Signed 128-bit intermediate arithmetic covers the unsigned 64-bit boot horizon. For balance b≤0, the wait until positive credit is floor(−b/Q)+1 windows. Take the maximum across each ready path and the minimum across paths to program the first actionable refill while idle. Work is bounded by the configured topology irrespective of missed periods. An unrepresentable future compare enters the failure path. Trusted task interrupt masking and platform delivery determine physical reaction latency. Product hard-preemption, isolation and FTTI need separate evidence.

The ordinary sample blocks leaf 0. Leaf 1 wakes it from another child under the same parent. Leaf 0 spins until the peer parent sets a flag and exits. Both siblings remain ready beneath their exhausted parent until the first actionable refill. Leaf 0 then completes and leaf 1 returns from wake. The late sample alone masks IRQ at task level for 0.3 counter seconds after restoration (1.2 periods). Independent replay requires eight rows, one idle-to-task recovery, at most eight periods per observation, ordinary and idle IRQ reaction ≤ P/8, and injected service crossing the first boundary before the second with parent debt ≥ P/2. Each raw row gives sampled end, previous and next compare, service charged, actual overrun, debts per level, next/previous owner, ready mask, and frame addresses. The host regression repeats a delayed entry on the recovered sibling and checks its next two-window wait, skips 10^14 periods, and checks the final representable counter value.

## 한국어

#21 하위 이슈 #84, CHG-040 / IMP-038, EVD-038 계획. 단일 코어 virt의 신뢰된 EL1 작업에 물리 카운터, CNTP 절대 시각, PPI30을 사용한다. 최대 구성은 부모 2개, 자식 3개, 리프 3개, 할당량 깊이 3이다. 각 노드의 양수 할당량 Q는 공통 고정 주기 P 이하다. 형제 중 작은 인덱스를 우선한다. block은 호출자를 차단하고 wake(id)는 차단된 리프만 깨우며 즉시 선점할 수 있다. 다른 상태나 잘못된 ID이면 false다. 반환은 영구 종료다. 준비 작업이 없으면 타이머를 끄고 WFI에 진입한다.

SVC나 타이머 IRQ를 관측하면 직전 디스패치부터 실제 관측까지의 시간을 해당 리프·자식·부모에 각각 한 번 부과한다. 조기 차단 시 미사용 예약을 환급한다. 계측 구간에는 전환 오버헤드가 포함된다. 예정 종료 이후의 실제 서비스도 청구하며 음수 잔액은 다음 주기로 넘어간다. 재충전 시 부채를 우선 상환하고 양수 잔액은 할당량을 넘지 않는다. 세 노드 모두 잔액이 양수가 되어야 다시 실행한다. 이전 소유자의 초과 시간을 동료나 유휴 작업에 부과하지 않는다. 첫 경계 이후 완전히 지난 주기당 P−Q를 빼므로 계산량은 누락된 주기 수와 무관하다. 노드 잔액 b≤0이면 양수 복구까지 floor(−b/Q)+1 주기가 필요하다. 각 경로의 최댓값을 구한 뒤 가장 이른 경로의 시각에 CNTP를 설정한다. 표현할 수 없는 미래 시각은 실패 경로로 보낸다. 제품의 반응 시간과 격리 한도는 별도 근거가 필요하다.

리프 0이 차단되고 다른 자식의 리프 1이 깨운다. 부모 할당량 소진 후 동료 부모가 실행되어 플래그를 쓰고 종료한다. 첫 복구 시점까지 유휴 대기한 뒤 두 형제가 복원된다. 지연 예제만 복원된 작업에서 IRQ를 1.2주기 마스킹한다. 기록에는 예정 종료, 실제 관측, 초과 시간, 세 노드 부채, 복원할 작업 및 프레임이 포함된다. 검증 기준은 기록 8개, 유휴 복귀 1회, 관측 간 최대 8주기, 일반 IRQ 및 유휴 복귀 지연 최대 P/8, 지연 사례의 최초 부모 부채 최소 P/2다. 호스트 회귀 사례는 복원된 형제의 반복 지연과 두 주기 대기, 10^14 주기 건너뛰기와 카운터 상한을 다룬다.

## Reproduction / 재현

From the repository root on Linux with QEMU and aarch64-linux-gnu-nm. Build from kernel/ to load the target configuration. 실제 게스트는 별도 경로에 캡처한다.

```sh
out=$(mktemp -d)
for app in ordinary-groups ordinary-groups-late; do
  (cd kernel && JNOX_PROFILE=groups JNOX_APP="$app" cargo build --locked --release --target-dir "$out/$app/plain")
  python3 tools/qemu_boot.py --profile "$app" --kernel "$out/$app/plain/aarch64-unknown-none/release/jnox-kernel"
  (cd kernel && JNOX_PROFILE=groups JNOX_APP="$app" JNOX_GROUP_TRACE=1 cargo build --locked --release --target-dir "$out/$app/trace")
  GROUP_CAPTURE_DIR="$out/$app/raw" python3 tools/qemu_boot.py --profile "$app" --kernel "$out/$app/trace/aarch64-unknown-none/release/jnox-kernel" --qemu tools/group_capture.py
done
python3 tools/group_replay.py --audit "$out/ordinary-groups/raw/audit.bin"
python3 tools/group_replay.py --late --audit "$out/ordinary-groups-late/raw/audit.bin"
python3 tools/group_replay.py --fixture
rustc --edition=2024 --test tools/hierarchy_tests.rs -o "$out/hierarchy-tests"
"$out/hierarchy-tests"
```

Retain revision, ELF hash, launch command, first UART, exception log, journal and terminal idle PC. Compare SVC #2, #3, #1 and PPI30 in the independent exception stream with the journal for VER-002/005/023. 출시 자료에는 리비전, ELF 해시, 실행 명령, 첫 UART, 예외 기록, 저널 및 유휴 PC를 남기고 SVC·IRQ를 대조한다.
