# EEVDF with live requests, reweighting and two wakes

[한국어](m4-eevdf.md) · [Change #56](https://github.com/Romanof123/JNOX/issues/56)

CHG-025 · POL-012 · REQ-002/005/023 → DES-003/012 → IMP-023 → VER-002/005/023. EVD-023 is the planned verification record. The table below gives the expected results for one Cortex-A72 virtual CPU, GICv2 and the nonsecure EL1 physical timer. The measured wake timestamps must come from the captured guest.

## Run the finite telemetry deployment

From the repository root on Linux or WSL, install Python 3 with pyelftools, Cargo and the kernel's pinned Rust toolchain with target `aarch64-unknown-none`, `qemu-system-aarch64`, and AArch64 GNU binutils (`aarch64-linux-gnu-nm` and `aarch64-linux-gnu-objdump`). Choose a new output directory. The command builds the locked release image and starts one default-clock QEMU virt/GICv2 guest with one Cortex-A72 and 128 MiB.

~~~sh
python3 tools/eevdf_telemetry.py run --out first-run
~~~

Keep the printed session SHA-256 separately. For offline replay, supply that recorded digest and the same run directory. Replay verifies the retained evidence without rebuilding or booting.

~~~sh
python3 tools/eevdf_telemetry.py replay --run first-run --session-sha256 DIGEST
~~~

`first-run/build.json` and `build.log` identify the build. Under `first-run/capture/`, inspect `uart.bin`, the frozen RAM journal `journal.bin`, `exceptions.log`, the ELF, symbols, disassembly and `session.json`. The run root contains `session.sha256` and `result.json`. The strict replay checks both batch workers’ counts, sums and weighted sums, the consumer tuple `(2,128,27028,115128)`, all 15 service comparisons and all eight wake responses. [Telemetry deployment criteria](eevdf-telemetry-deployment.en-ko.md)

`QUALIFIED` exits 0. `INVALID` exits 1 for a failed check, including a wrong digest. `UNOBSERVED` exits 2 when required evidence is unavailable. A build, tool or launch error before a session exists prints `ERROR` and exits 1. Retain the entire first directory, including partial files from a failed attempt. Use a different directory for any later launch.

## Selection and request changes

For task weight `w`, let `n/w` be its virtual runtime. At each decision the fair clock is exactly `V = sum(n) / sum(w)` over ready tasks. A task is eligible when `n/w ≤ V`. Among eligible tasks the earliest virtual deadline wins. A tie goes to the smaller task ID. A request starting at numerator `s` with slice `q` has deadline `(s+q)/w`. Charging one physical service unit increments `n` by one. Partial service keeps that deadline. Exhaustion renews the request at the old deadline. [Selection](../kernel/src/fair_policy.rs) · [Live request policy](../kernel/src/eevdf_policy.rs)

Configuration changes occur between dispatches. A changed slice starts a fresh request at the current virtual runtime. To change weight from `w` to `w′`, the policy starts at numerator `ceil(n + (w′−w) V)`. This preserves signed service lag relative to the old clock within one service unit of upward rounding. A wake places a sleeper at `max(n, ceil(w V))` and shifts its deadline by the same numerator difference, preserving remaining service. The last runnable task's clock supplies the placement floor after idle.

## Twelve grants

A, B and C have IDs 0, 1 and 2. They start ready at virtual time zero with weights `[1,2,1]` and slices `[1,8,4]`. One service unit is `Q = floor(CNTFRQ_EL0 / 100)` counter increments. Each grant programs one compare value and ends at a physical timer IRQ. The two subsequent sleep syscalls run in untimed control intervals. [Guest](../kernel/src/eevdf.rs) · [Task loops](../kernel/src/eevdf.S)

| Row | Change before selection | Ready | V | Task × units | Cumulative service error × 20 (A, B, C) |
|---:|---|---|---|---|---|
| 0 | Initial dispatch | ABC | 0/4 | A × 1 | 15, −10, −5 |
| 1 | A has renewed | ABC | 1/4 | B × 1 | 10, 0, −10 |
| 2 | C's slice changes from 4 to 2 | ABC | 2/4 | C × 1 | 5, −10, 5 |
| 3 | C sleeps. A's weight and slice change from (1,1) to (2,2) | AB | 3/4 | B × 3 | −25, 20, 5 |
| 4 | C wakes with one unit remaining | ABC | 8/5 | A × 2 | −1, 4, −3 |
| 5 | Equal runtime, A wins the deadline tie with C | ABC | 10/5 | A × 2 | 23, −12, −11 |
| 6 | C resumes after its first sleep | ABC | 12/5 | C × 1 | 15, −20, 5 |
| 7 | C sleeps with a renewed two-unit request | AB | 10/4 | B × 1 | 5, −10, 5 |
| 8 | C wakes and keeps its existing placement | ABC | 14/5 | B × 1 | −3, 2, 1 |
| 9 | A wins the deadline tie with B | ABC | 15/5 | A × 2 | 21, −14, −7 |
| 10 | B completes its original eight-unit request | ABC | 17/5 | B × 2 | 5, 10, −15 |
| 11 | C resumes after its second sleep | ABC | 19/5 | C × 1 | −3, 2, 1 |

Row 1 distinguishes eligibility from sorting by deadline alone. A's deadline is 2, earlier than B's and C's 4, but A's runtime 1 exceeds V of 1/4. B wins the eligible tie. At row 2, B's runtime equals V of 1/2 and C's new deadline of 2 moves it first. With C asleep, reweighting A at V of 2/3 gives it numerator `ceil(1 + 2/3) = 2`. The first wake uses the pre-wake V of 6/4 and moves C's (runtime, deadline) from (1, 2) to (2, 3). The second wake uses V of 11/4 and leaves C at runtime 3, deadline 5.

For each grant, credit its owner with actual service and subtract `units × weight / total ready weight` from every ready task's balance. The totals over 18 units are A 7, B 8 and C 3. The largest absolute cumulative error is 25/20 units, and the final errors are `[−3,2,1]/20`. The two wake-to-dispatch waits are 4 and 5 service units. These bounds and samples describe this workload. Product timing limits and physical-board response require separate measurements.

## Public journal and acceptance

`eevdf_witness` contains 4,000 bytes, or 500 little-endian 64-bit words. Row numbers and field offsets below start at zero. The signed error fields use two's complement.

| Records | Word layout within each record |
|---|---|
| Header at word 0, 8 words | Version 1, row count 12, sleep count 2, response count 2, counter frequency, Q, total service 18, peak absolute error in twentieths 25. |
| 12 rows starting at word 8, 39 words each | 0–4 owner, grant, ready bit mask, V numerator and denominator. 5–16 hold the three pre-dispatch task tuples. 17–23 are dispatch counter, programmed compare, IRQ counter, interrupted PC, saved frame address, restored PC and task progress. 24–35 hold the post-charge tuples. 36–38 hold signed cumulative errors × 20. Each task tuple gives runtime numerator, deadline numerator, weight and remaining physical service. |
| 2 wake records starting at word 476, 12 words each | 0–3 preceding compute row, SVC counter, SVC return PC and SVC frame. 4–7 row before which C wakes, wake counter and pre-wake V numerator/denominator. 8–11 first resumed row, cumulative service at that dispatch, counter first read in C after return, and service units waited. |

Reconstruct all 12 selections from the captured tuples using exact fractions. In the same boot's exception trace, match 12 PPI 30 IRQs to the interrupted task loops and both `SVC #1` calls to C. Follow the actual exception returns (`ERET`) to the selected PCs. On both resumes C must return to `ee_c_wake` and read `CNTPCT_EL0` in task code. Check private frame addresses and advancing work counters against the ELF. For each row require `compare = start + Q × grant` and `compare ≤ IRQ < compare + Q`. For each wake require `wake_at ≤ start ≤ first_work < compare` in the resumed row. Record both `first_work − wake_at` samples in counter ticks and convert using the captured frequency. Keep these observed latencies alongside the 4- and 5-unit scheduling waits.

Host controls cover invalid task IDs and parameters, duplicate wakes and stops, wrong-task and excess charges, mutation during a dispatch, a negative projected start and timestamp overflow. Each rejected operation must preserve the open request and charge. A mismatched owner, an ineligible winner, stale progress, a missing SVC or an unmatched return PC invalidates the guest evidence. [Exact host controls](../tools/eevdf_tests.rs) · [Horizon control](../kernel/src/eevdf_policy.rs)

## Replay

From the repository root on Linux with the kernel toolchain, QEMU and AArch64 binutils installed, capture the first boot in a fresh directory. The wrapper forwards the launcher's arguments and enables the exception trace and QMP journal dump. [Launcher](../tools/qemu_boot.py) · [Capture](../tools/eevdf_capture.py)

```sh
(cd kernel && cargo build --release --locked --features eevdf-bench -j1)
run_dir=$(mktemp -d)
export EEVDF_CAPTURE_DIR="$run_dir/capture"
cat > "$run_dir/qemu-eevdf" <<'SH'
#!/bin/sh
exec python3 -B tools/eevdf_capture.py "$@"
SH
chmod +x "$run_dir/qemu-eevdf"
python3 tools/qemu_boot.py --profile eevdf --runs 1 \
  --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel \
  --qemu "$run_dir/qemu-eevdf"
```

Retain source and ELF hashes with `launch.json`, `uart.bin`, `int.trace`, `audit.bin`, `qmp.json` and `exit.json` from the same launch. After the two boot lines the expected EEVDF milestones are `BEGIN`, `EARLY INELIGIBLE`, `SLICE CHANGE`, `WEIGHT CHANGE`, `TWO WAKES`, `STOP` and `PASS`. Preserve the first capture even if a later replay is necessary.

The independent virtual-guest results and frozen image are recorded in [EVD-023](evidence/evd-023.en.md)
