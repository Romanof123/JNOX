# Finite recurring service for ordinary EDF and sampled LLF / 유한 반복 서비스

Prospective CHG-104 / IMP-104 / EVD-104 · POL-005 · [#273](https://github.com/Romanof123/JNOX/issues/273) under [#21](https://github.com/Romanof123/JNOX/issues/21). Design baseline c7d09f866d2b535364a7e2a6cbd1a7a5b8d5866c. All results below are acceptance targets.

## Admission and fixtures

Use integer cells k in [0,H), H=16, Q=counter frequency/10 and the original absolute origin S. Validate 0<=offset<H, 1<=width<=period, period>0. For k>=offset reserve exactly when (k-offset)%period<width. Clip the last occurrence to H. No bit H is reserved. Compile one checked mask before launch and retain it in runtime for both admission and dispatch. Enumerate starts independently of mask edges so adjacent occurrences (width=period) remain distinct. Normal runtime evidence covers the fixture below. Arbitrary calendars have a slot admission model, without a general service execution-time guarantee.

| Fixture | Jobs (release, deadline, cost in Q cells) | Expected result |
|---|---|---|
| Normal | A (0,13,9), B (10,16,1) | offset=1, period=8, width=1 reserves cells 1,9 (mask 514). In both policy models A receives 0,2–8,10 and B receives 11. |
| Supply refusal | A (0,10,9) | Full supply passes. With only cell 1 withheld, nine cells remain. With both reserved, [0,10) has eight. EDF refuses Demand, sampled LLF refuses Miss. |
| Owner refusal | Normal table | Full and reserved supply pass. Ceiling 3 refuses, 4 admits, before context construction, compare programming or worker/service entry. |

Use existing checked job validation, release/deadline demand intervals for EDF and deterministic one-cell scheduler replay for both policies, including withheld-cell releases and deadline checks before work at the deadline. Exercise small independent exhaustive calendars, partial final occurrences, width=period, empty intersections, invalid inputs and overflow. Cross-check every interval's supplied cells and both policy schedules against an independent enumerator.

## Execution and return

The one-shot supply_service writes one sum then idles. Its parked frame cannot perform a second invocation. In the separate recurring profile dispatch the service stack at the first grid IRQ of each occurrence using a fresh entry frame with its ordinal; reinitialise only after the previous activation is inactive and its audit retained. Within an occurrence reuse its frame. A service wrapper calls a real out-of-line reducer on a distinct 64-element input batch (values 1..64 then 65..128), records its returned sum, ordinal, input range, SP and return counter in separate records, then waits for the boundary IRQ. Keep compiled call/return inspectable. Expected sums 89440 and 617824. At the outgoing IRQ require the proper ordinal and completed return strictly before the absolute end; early finish retains the entire reserved slot. No service SVC or asynchronous release. Adjacent windows would require switching parked activations from the handler stack; never overwrite a live exception frame while using its contents. Restrict guest selectors to the stated fixture.

A runs the existing uninterrupted arithmetic loop with absolute stop S+10.5Q (1050 ms from S); B uses 30 ms from its first entry. Check the stop arithmetic against frequency divisibility. A saves its ongoing state at cells 1 and 9, restoring that same frame at 2 and 10. Require positive progress before the first IRQ, frozen while withheld, strictly larger progress before the second IRQ and again before completion. Verify both worker products by recomputing the wrapping recurrence for the observed block counts, using initial value id+1 and 64 updates per block. B must finish. Check saved/selected frame addresses, ELR, disjoint stack ranges, actual successor owner and restore counters; retain both return records until terminal. Handoff and completion rows must reconstruct both service interruptions even when trace printing is off. Fail on incomplete activations, post-grid SVC, late compare, deadline violation, truncated evidence or terminal before all required returns. Keep all compares at S+kQ; a worker completion may dispatch B within the same cell. Model costs are whole cells; actual completion timestamps need not equal the modeled slot ends.

## Bounds and assessment

Reuse EVD-100 under the existing one IRQ in each assigned cell and one completion SVC per one-shot job assumptions. For aligned cells at most one grid entry plus two worker completions; for every wholly contained integer half-open window of W counts in [S,S+17Q), at most min(17,1+ceil((W-1)/Q))+2 entries and no more returning owner changes. W=Q and Q>1 gives 4. Count actual different-owner returns including idle launch, excluding terminal park. Service function returns introduce zero exception entries. Distinguish the two reserved *logical* slots from measured counter occupancy; record IRQ delivery offsets against S+kQ and observed owner intervals. Exact IRQ-at-boundary and physical WCET are outside this fixture.

For each policy run six new positions: accepted ceiling 4 plain/trace, refused ceiling 3 plain/trace and supply-refused plain/trace (use 4). Capture full UART, frozen RAM evidence including refusal counters/zero frame launches, ELF, build configuration, source identity and terminal PC per guest. Reconstruct slot owners, all sliding window peaks and products independently. Preserve failed attempts. Replay all existing deadline/supply/cap entry points against the baseline, comparing terminal behavior and reporting text/load drift. Use distinct selectors ordinary-{edf,llf}-recurring and ordinary-{edf,llf}-recurring-refuse, and existing trace and owner-ceiling switches. First dependencies are selector/build routing, checked calendar compilation and persistent per-occurrence records. Extend completion/terminal and cap scoring only in the recurring path. Keep the original SUPPLIED, TIGHT, RESERVED and one-shot service behavior intact. No IPC dependency.

## 한국어 실행·판정 계약

기준은 단일 CPU, H=16, Q=주파수/10, 원래 절대 원점 S이다. 0<=offset<H, 1<=width<=period를 검사한다. offset 이후 (k-offset)%period<width인 셀을 예약하고 마지막 구간은 H에서 자른다. 한 번 계산한 마스크를 승인과 실제 배치가 공유한다. 폭과 주기가 같을 때도 각 호출의 시작은 별도로 계산한다. 일반 달력은 슬롯 모델로 검증하며 실제 게스트 범위는 위 표의 구성이다.

정상 예약은 1,9번 셀이며 A(0,13,9), B(10,16,1)는 두 정책에서 승인된다. 모델의 A 슬롯은 0,2–8,10, B 슬롯은 11이다. 거부용 A(0,10,9)는 첫 예약만 있으면 통과하고 둘 다 있으면 공급 8로 부족해 EDF Demand, LLF Miss가 된다. 정상표에서 한도 3은 프레임 생성·타이머 시동 전에 거부되고 4는 통과한다. 독립 열거로 잘린 구간, 연속 호출, 잘못된 값, 오버플로, 모든 구간의 공급량 및 정책 재생을 대조한다.

각 호출은 독립 서비스 스택의 새 진입 프레임에서 서로 다른 64개 입력을 제곱 합산한다. 호출자가 실제 반환값 89440, 617824와 호출 번호·입력 범위·SP·반환 카운터를 보관한다. 예약 끝 IRQ까지 대기하며 완료 SVC를 추가하지 않는다. 이전 호출이 끝나고 실행 스택에서 벗어난 뒤에만 프레임을 재사용한다. A의 절대 정지는 S+10.5Q이다. 1→2와 9→10에서 저장·복원한 A 프레임, ELR, 복원 시각, 스택, 소유자 및 연산 진행량을 확인한다. 예약 중 진행량은 일정하고 두 예약 사이와 두 번째 복귀 뒤에는 증가해야 한다. 블록 수로 두 작업의 누적 결과를 독립 재계산한다. B는 시작 후 30 ms 동안 실행한다. 모든 비교 시각은 S+kQ이며 조기 SVC는 같은 셀에서 다음 작업을 배치할 수 있다.

정렬된 셀의 예외 진입은 최대 3, Q폭 임의 정수 이동 구간은 최대 4이다. 서비스 함수 반환은 예외가 아니다. 실제 다른 소유자로 복귀한 경우만 전환으로 센다. 미복귀 종료, 예약 셀과 실제 점유 시각을 구분한다. IRQ 셀 위반, 마감 위반, 누락된 반환이나 증거는 실패이다. 각 정책의 정상·한도 거부·공급 거부를 각각 일반/추적으로 실행해 총 12개 게스트의 UART·RAM·ELF·종료 PC를 보존하고 기존 진입점도 기준과 대조한다. 초기 의존성은 선택기 배선, 공용 달력 판정, 호출별 반환 기록이다. 기존 단일 예약 경로는 유지한다. 실제 하드웨어 최악 시간은 별도 검증 대상이다.

[Finite supply / 유한 공급](evidence/evd-095.en.md) · [Owner bounds / 전환 한도](deadline-owner-cap-plan.en-ko.md) · [Runtime](../kernel/src/ordinary_deadline.rs) · [Admission](../kernel/src/deadline_jobs.rs)

## Source implementation and capture / 구현 및 수집 절차

From a clean Linux/WSL checkout with Cargo's aarch64-unknown-none target, QEMU and aarch64-linux-gnu binutils installed, use a new absolute evidence directory for each attempt. The wrapper receives SIGTERM after ten seconds, freezes the guest, verifies the terminal PC range and reads the ELF-named audit records. GNU timeout signals only the wrapper with --foreground. Inspect capture/exit.json and registers.txt as well as the terminal UART. Select cap 4 or 3 on the normal application, or cap 4 on its -refuse variant. Set JNOX_DEADLINE_TRACE=1 for the corresponding trace capture.

깨끗한 Linux/WSL 환경에서 각 시도마다 새 증거 디렉터리를 사용한다. 일반 앱에 한도 4/3, 공급 거부 앱에 한도 4를 지정하고 각각 일반/추적 모드로 실행한다. 아래 예시는 EDF 승인 구성이다. 래퍼는 종료 위치에서 RAM을 동결하며 stdout, UART, 예외 기록과 심볼별 원본을 남긴다.

~~~sh
set -eu
root=$(pwd -P)
out=$(mktemp -d)
export JNOX_BOARD=virt JNOX_PROFILE=deadline JNOX_APP=ordinary-edf-recurring
export JNOX_DEADLINE_OWNER_CAP=4
unset JNOX_DEADLINE_TRACE
export JNOX_RECURRING_CAPTURE="$out/capture"
git rev-parse HEAD > "$out/source"
env | grep '^JNOX_' | sort > "$out/selectors"
(cd kernel && CARGO_TARGET_DIR="$out/target" cargo build --locked --release -j1) > "$out/build.log" 2>&1
cp "$out/target/aarch64-unknown-none/release/jnox-kernel" "$out/kernel.elf"
aarch64-linux-gnu-objdump -d "$out/kernel.elf" > "$out/disassembly"
timeout --foreground --preserve-status -k 4s 10s python3 "$root/tools/deadline_recurring_capture.py" \
  -machine virt,gic-version=2 -cpu cortex-a72 -accel tcg -smp 1 -m 128M \
  -nographic -nic none -monitor none -serial stdio -kernel "$out/kernel.elf" \
  < /dev/null > "$out/uart.raw" 2> "$out/stderr"
(cd "$out" && sha256sum kernel.elf uart.raw capture/*.bin > SHA256SUMS)
~~~

CALENDAR gives offset/period/width/mask and the first-reservation-only verdict. RECURRING_JOB gives id/release/deadline/cost/milliseconds before admission. CONFIG gives timer frequency, absolute origin and Q. audit.bin holds two rows of six little-endian u64 words (ordinal+1, first/last input, SP, sum, return counter). deadline_recurring_launch.bin counts frame initializations, timer compares programmed and the initial restore call. All three are zero on either refusal. For normals, decode D/J dispatch rows against the saved RAM, verify both worker products independently using the recorded block counts and locate the reducer call/return in disassembly.
