# Energy- and deadline-aware placement on two guest CPUs

[한국어](m3-power-placement.md) · [Issue #58](https://github.com/Romanof123/JNOX/issues/58)

CHG-027 · POL-017 · IMP-025. This acceptance procedure defines the evidence to collect for EVD-025. On QEMU virt, two Cortex-A72 vCPUs execute the same 4,096-iteration stack-based sum (8,390,656) on the selected CPU. OPP identifiers, capacity, thermal limits, wake cost and energy are model inputs. MPIDR, stack pointer, result and generic-counter samples come from guest execution.

## Workload and expected decisions

Each request starts at model tick 0 with 12 work units, minimum capacity 1, deadline 6 and protected margin 1. Both CPUs are available, with zero capacity reserve. The usual affinity mask is 3. Effective capacity is the smaller of nominal and thermal capacity, less shared load and capacity reserve, floored at zero. Run ticks round work divided by capacity up to the next integer. Wake plus run must fit by tick 5. Choose the feasible point with least modeled energy (wake energy plus work × energy per work), breaking ties by CPU then OPP.

| CPU / OPP | Capacity (work/tick) | Energy/work | Wake energy | Wake ticks |
|---|---:|---:|---:|---:|
| 0 / 0 | 3 | 1 | 1 | 1 |
| 1 / 1 | 6 | 3 | 2 | 1 |

| Row | Input change | Expected decision | Run / wake / deadline margin / energy (model) |
|---:|---|---|---|
| 0 Base | Thermal caps 3 and 6 | Model, CPU0/OPP0 | 4 / 1 / 1 / 13 |
| 1 Thermal | CPU0 thermal cap 2 | Model, CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 2 Shared | CPU0 shared load 1 | Model, CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 3 Deadline | Both thermal caps 1 | Predicted deadline miss, no dispatch | — |
| 4 Fallback | Empty catalogue, trusted CPU1/OPP1 supplied | Feasible fallback, CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 5 Idle | Empty affinity mask | Idle, no dispatch | — |

In row 2 CPU0 finishes one background sum before dispatch. Halfway through its sum CPU1 publishes gate 2 and waits. CPU0 runs a second sum, then publishes gate 3. After observing CPU1's completion, CPU0 runs a third sum. Acquire/release operations order the middle batch inside the foreground interval. The measured response includes that wait and the first batch. The model input of one unit expresses the assumed capacity reduction.

## Reproduce and verify

From the repository root on Linux, use a new capture directory for each run. The observer selects two vCPUs, GICv2 and multithreaded TCG.
```sh
(cd kernel && cargo build --locked --release --features power-bench -j1)
elf=kernel/target/aarch64-unknown-none/release/jnox-kernel
wrap=$(mktemp)
printf '#!/bin/sh\nexec python3 "%s/tools/power_capture.py" "$@"\n' "$PWD" > "$wrap"
chmod +x "$wrap"
POWER_CAPTURE_DIR="$PWD/power-capture-01" python3 tools/qemu_boot.py --profile power --kernel "$elf" --qemu "$wrap"
```

Retain the ELF, revision, SHA-256, tool versions, launch arguments, UART bytes and QMP dump together. Require the following transcript within five seconds, 100 ms of quiet and host-initiated termination.
```text
JNOX M1 BOOT
JNOX M1 READY
POWER BASE
POWER THERMAL
POWER SHARED
POWER DEADLINE MISS
POWER FALLBACK
POWER IDLE
POWER PASS
```

Resolve `power_witness` with `aarch64-linux-gnu-nm -S` on that ELF. Independently check `launch.json`, `qmp.json`, `audit.bin`, `int.trace`, `uart.bin` and `exit.json`. QMP must report a running guest with CPU indices 0 and 1. The physical dump must match the symbol address and size. Decode little-endian 64-bit words. The eight header words are ABI version 1, six published rows, completion flag 1, CNTFRQ, model tick length (CNTFRQ / 10 rounded down), iteration count 4096, expected sum 8390656 and remote job count 3. Row i starts at word 8 + 34 × i. The 212 payload words occupy 1,696 bytes; the 64-byte-aligned object includes trailing padding.

```text
 0 case                 1 decision             2 model_points        3 allowed_mask
 4 work                 5 min_capacity         6 deadline_ticks      7 deadline_reserve
 8 cpu0_thermal         9 cpu0_shared         10 cpu1_thermal       11 selected_cpu
12 opp                 13 capacity            14 run_ticks          15 wake_ticks
16 model_margin        17 model_energy        18 release            19 dispatch
20 work_start          21 work_end            22 observed_complete  23 deadline_counter
24 measured_margin     25 mpidr               26 sp                 27 result
28 load_start          29 load_end            30 load_batches       31 load_result
32 dispatch_to_work    33 response
```

Decision codes 1–4 denote model placement, fallback, predicted miss and idle. In rows 3 and 5 selected_cpu is UINT64_MAX and the dispatch, work, result and measured timing fields remain zero. In every row deadline_counter equals release + 6 × header tick length. For dispatched jobs verify CPU affinity bits in MPIDR, aligned nonzero SP, identical sum and release ≤ dispatch ≤ work_start < work_end ≤ observed_complete. Recompute dispatch_to_work, response and signed measured_margin as work_start − dispatch, observed_complete − release and deadline_counter − observed_complete respectively. Field 24 encodes the difference modulo 2⁶⁴. Divide counter deltas by CNTFRQ for seconds. In row 2 check three background batches, the same sum, and a load interval enclosing the foreground interval. Other rows have zero load fields.

For an independent instruction check, resolve `power_work_loop` and `power_work_end` from the same ELF and inspect the disassembly. In a separate instrumented replay collect vCPU/thread, PC and SP at loop entry on each invocation. Expect one foreground call on CPU0, three on CPU1 and three background calls on CPU0, with the loop PC inside the symbol range and an aligned CPU-local stack. Use that replay for control flow. Use the uninterrupted run for timing. The QMP CPU inventory and exception log alone do not sample ordinary loop PCs.

## Physical board calibration

Record board revision, firmware, governor, cooling, supply, ambient conditions and workload identity. For each allowed CPU/OPP and idle state, confirm the actual OPP and temperature, repeat the identical workload, and timestamp release, wake request, first work and completion. Synchronize an external power meter or shunt trace with those events, record voltage/current sample times and integrate power over stated windows with a documented idle baseline. Repeat under controlled competing load and observed thermal throttling. Preserve raw traces, calibration uncertainty, counter frequency, measured capacity, wake latency, energy, response margins and observed deadline misses. Compare against the declared board profile's criteria. Physical energy, wake latency, OPP transitions, temperature and board deadline behavior require these measurements.

The independent virtual-guest results and frozen image are recorded in [EVD-025](evidence/evd-025.en.md)

## Offline reduction record (#243)

Use one directory per acquisition. Preserve the original export bytes and producer/custody record. Pass its manifest to `python3 tools/power_reduce.py MANIFEST`. JSON input is UTF-8. The manifest names the producer (`producer`), class (`kind`, either `fixture` or `board`), and two objects `events` and `samples`, each with a sibling filename `file` and lowercase `sha256` of the unmodified bytes. Record the manifest digest independently at acquisition. Digests identify supplied bytes. Authenticity requires the original acquisition record and independent custody.

The events JSON carries nonempty text fields `board` (revision), `firmware` (image identity), `workload` (image hash, recipe and observed result), `cpu_opp` (requested and observed), `thermal` (temperature and throttling), `shared_load`, `environment` (governor, cooling, supply and ambient), `idle_state`, `instrument` (meter, calibration identity and voltage/current uncertainty), `trigger` (common edge and synchronization method), and `limits` (named board profile and condition-specific limits, or explicitly unavailable). Record each allowed CPU/OPP, idle state, observed throttling and controlled shared load separately. Retain original instrumentation and condition logs with the acquisition.

Use integer counter values `counter_hz`, `trigger_tick`, `release_tick`, `wake_tick`, `first_tick`, `complete_tick`, `deadline_tick`, `idle_start_tick`, `idle_end_tick`. The same physical edge is `trigger_ns` on the meter. Map each tick to trigger_ns + (tick − trigger_tick) × 10⁹ / counter_hz. Use one monotonic unwrapped counter and constant frequency. Record integer `sync_uncertainty_ns` covering alignment and drift over this record, and prospective `max_gap_ns` bounding accepted adjacent sample spacing. The samples JSON is an ordered array of integer triples [meter ns, microvolts, microamps], with positive current denoting consumption. Limit each input to 4 MiB, 100,000 samples and integers of absolute magnitude below 2⁶⁴.

Interpolate sampled instantaneous power V × I linearly between neighboring times, clip trapezoids at window edges and integrate in joules. Cover release through completion and a disjoint, matched idle window. A window requires both edges bracketed and every contributing gap within max_gap_ns. Report exact covered duration and fail incomplete coverage without extrapolation. Subtract idle average power × run duration from gross run energy and retain the sign. Wake is first work minus wake request, response is completion minus release, and margin is deadline minus completion (seconds). A negative observed margin records a miss. Calibration, synchronization and interpolation limit physical accuracy. Keep repeats and raw traces for uncertainty evaluation and product limits under #34 / #1. Arithmetic fixtures and EVD-025 establish no physical limit.

[Independent known-answer fixture results and replay archive](evidence/synchronized-power-reducer-first.en-ko.md).
