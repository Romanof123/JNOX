# CHG-103 · Finite seeded lottery admission / 유한 시드 복권 수용

Prospective contract on ec00258 (#269, #21, PM #6). POL-010 · REQ-002/005/023 → DES-003/012 → VER-002/005/023 · IMP-103/EVD-103. Existing ordinary and assembly firsts including #79 retain their separate [source and evidence](ordinary-proportional-plan.md).

## Configuration / 공개 구성

Single QEMU virt Cortex-A72/GICv2, 128 MiB, TCG, physical PPI30 and 100 Hz slots (counter F divisible by 100). JNOX_BOARD=virt, JNOX_PROFILE=proportional, explicit JNOX_APP below, no bench features or JNOX_CONTROL_CASE. Three ordinary privately stacked Rust workers, stable IDs 0/1/2, continuously ready mask 7, weights [1,2,4]. H=48 actual task dispatches, W=24. Check every half-open [s,s+24), s=0..24 inclusive, with zero-based dispatch indices. Grant count is a slot occupancy measure. Log actual entry/start, programmed compare, stop counter, elapsed counter ticks and return/context evidence separately. No WCET or deadline guarantee.

| JNOX_APP | seed | required by ID | exact window minima | expected |
|---|---:|---|---|---|
| lottery-window1 | 1 | 5,3,11 | 5,3,11 | admit, totals 11,14,23 |
| lottery-window2 | 2 | 1,7,10 | 1,7,10 | admit, totals 8,17,23 |
| lottery-window-tight | 1 | 6,3,11 | 5,3,11 | first failure [24,48), ID 0, 5 < 6 |

## Gate, workload and evidence

Before frame launch or timer enable, validate H in 1..64, W in 1..H, fixed N=3 and mask=7, each weight in 1..16, each floor in 1..W, sum of floors ≤ W. Fixed array storage, checked indices/count arithmetic, bounded rejection sampling. Evaluate a separately initialized policy using actual proportional_policy.rs dispatch/stop, SplitMix64 cold seed, rejection below (2^64 mod total), then ascending-ID weighted ticket intervals. Report observed minima and earliest violating (start,ID) in lexicographic order. Report live state before/after gate for either outcome. Refusal launches zero frames and consumes zero live words. After admission the scheduler calls live dispatch afresh each slot; forecast produces no runtime winner list. No entropy claim or extension beyond the declared horizon.

Application: three packet telemetry streams. Each grant consumes the next 16-byte input block for its stream; byte j = (37*ID + 11*ordinal + j) mod 256, ordinal starts at zero per stream. ID 0 computes payload byte sum, ID 1 counts bytes with low two bits zero, ID 2 computes maximum byte. Worker publishes (slot,ID,ordinal,result) once before waiting for the timer; successive grants to the same worker require distinct receipts. Withhold qualification for missing/duplicate/stale receipts or early preemption. Independent replay regenerates every raw word, rejected prefix, ticket, interval, winner, every window, minima and block receipt. Join actual IRQ/PPI, ERET, private stack and continuation to grant; retain first boot/timeout, raw UART, disassembly, ELF/source hash and exact QEMU argv. Refusal must lack grants, returns, worker results and timer IRQ.

Controls: zero/oversize horizon, zero/oversize window, zero/oversize weight, partial/foreign ready mask, zero/impossible floor, excessive sum, boundary W=1 and W=H, equal minimum vs one tighter; prior ordinary lottery1, lottery2, lottery-miss, stride, stride-delay and proportional-bench remain individually attributable. Source registry: kernel/src/proportional_policy.rs owns draws, ordinary_proportional.rs owns actual lifecycle, kernel/build.rs selects profiles. Product seed/readiness, workload inputs and board timing remain #1/#10/#34.

## 한국어 계약

단일 코어에서 세 작업이 항상 준비되고 표의 시드·가중치·48회 실행이 유지될 때만 표의 최솟값을 보장한다. 0부터 24까지 각 시작점의 반열린 24슬롯 창을 전부 검사한다. 위 입력 범위와 합계를 검사한 뒤 별도 정책 인스턴스로 실제 추첨 함수를 호출한다. 실제 상태는 분석 전후에 같아야 하며 수용한 경우 매번 새로 추첨한다. 거절 시 최초 위반 창과 작업을 남기고 타이머와 작업을 시작하지 않는다.

세 패킷 스트림에서 각 배정은 해당 작업의 다음 16바이트 블록을 처리한다. 작업별 바이트 합계·하위 두 비트가 0인 바이트 수·최댓값을 슬롯, 작업 번호, 블록 순번과 함께 한 번씩 게시한다. 독립 검증은 원시 난수와 거부값, 티켓, 실제 복귀와 IRQ, 모든 창과 작업 결과를 재계산한다. 비교 레지스터, 경과 카운터와 점유 슬롯 수를 별도로 기록한다. 최악 실행시간이나 보드 마감시간은 보장 범위 밖이다. 표의 두 수용과 한 단위 강화 거절, 잘못된 입력, 경계값 및 기존 다섯 앱과 어셈블리 벤치의 최초 근거를 구분해 보존한다.

## Source capture / 최초 실행 보관

From the root of a clean native Linux checkout run the following once per position, in table order. Set CASE to the exact selector and OUT to a fresh absolute directory on native Linux storage. A terminal row alone is insufficient for verification. The kernel parks after printing it. Retain all files on error; a retry gets a new directory. / 깨끗한 Linux 소스에서 표 순서대로 CASE와 새 절대경로 OUT을 지정한다. 최초 실패와 후속 실행은 서로 다른 경로에 보존한다.

```bash
set -eu
: "${CASE:?supply selector}" "${OUT:?supply new absolute directory}"
mkdir "$OUT"
exec >"$OUT/run.log" 2>&1
git rev-parse HEAD >"$OUT/source"
git status --porcelain >"$OUT/status"
cd kernel
rustc -Vv
cargo -V
qemu-system-aarch64 --version
env -u JNOX_CONTROL_CASE JNOX_BOARD=virt JNOX_PROFILE=proportional JNOX_APP="$CASE" \
  CARGO_TARGET_DIR="$OUT/build" cargo build --release
cp "$OUT/build/aarch64-unknown-none/release/jnox-kernel" "$OUT/kernel.elf"
sha256sum "$OUT/kernel.elf" >"$OUT/sha256"
llvm-objdump -d "$OUT/kernel.elf" >"$OUT/disassembly"
args=(-machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -m 128M -accel tcg -nodefaults
      -display none -monitor none -serial "file:$OUT/uart" -d int -D "$OUT/exceptions" -kernel "$OUT/kernel.elf")
printf '%q ' qemu-system-aarch64 "${args[@]}" >"$OUT/argv"
qemu-system-aarch64 "${args[@]}" & pid=$!
trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT
for ((i=0;i<600;i++)); do
  if grep -Eq '^PROP END ([0-9]+ ){4}[0-9]+$' "$OUT/uart" 2>/dev/null; then exit 0; fi
  kill -0 "$pid" || exit 1
  sleep 0.1
done
echo "UART terminal timeout" >&2
exit 124
```

The [ordinary trace dictionary](ordinary-proportional-plan.md) applies. Additional decimal rows use these six fields in order. Row 15 records mode (5/6/7), seed, H, W, ready mask, packed weights. Row 16 records three floors, actual state before/after, admission flag. Row 17 records three minima and three zeros. Row 18 records the first insufficient start, exclusive end, ID, count, floor, zero. Row 19 records the worker's own slot, ID, zero-based packet ordinal, result, publication counter, private local address. Existing row 12 contains per-stream completed block count, last result, timestamp and address for each window-mode slot. Cold initial values are count zero, placeholder ID+1 and zero timestamp/address. Row 7 starts/arms the grant, row 8 supplies the return and stop samples, rows 9–10 provide whole ticks and remainder. A delayed IRQ may extend measured residency without creating a second grant. Row 13 includes one initial idle timer IRQ; slot sentinel is u64::MAX. Initial idle return uses slot label zero and ID 3. Position 3 (tight) expects only rows 15–18 followed by PROP END 0 0 0 0 0. No frame, IRQ or task rows may precede or follow refusal.

Source controls from repo root: `rustc +1.95.0 --edition 2024 --test tools/scheduling_tests.rs -o /tmp/lottery-host` then `/tmp/lottery-host finite_windows_cover_edges_and_reject_unsupported_contracts --test-threads=1`. These are author arithmetic checks, separate from independently written replay. W=1 cannot satisfy three strictly positive floors; W=H covers exactly one window.
