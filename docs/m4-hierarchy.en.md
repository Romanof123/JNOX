# Hierarchical CPU budgets on one virtual CPU

[한국어](m4-hierarchy.md) · [Change #57](https://github.com/Romanof123/JNOX/issues/57)

CHG-026 · POL-016 · IMP-024 · planned evidence EVD-024. This profile uses one Cortex-A72 virtual CPU, GICv2 and the EL1 physical timer on QEMU virt.

## Two six-slot windows

Each of four leaves starts on its private stack and blocks through SVC #2. The first IRQ interrupts idle and releases leaves 1, 2 and 3 together. Leaf 0 stays asleep despite its higher-priority branch having budget. The ready mask remains 0xE throughout. Slot 0 opens with full budgets and slot 6 replenishes them on the original grid. The IRQ at boundary 12 stops the timer.

Parent budgets are [3, 6]. Parent 0 owns children 0, 1 and 2 with budgets 6, 1 and 4. Parent 1 owns child 3 with budget 6. Each child contains the same-numbered leaf with budget 6. For a six-slot request the scheduler selects the lowest eligible (parent, child, leaf) tuple. The requested length, time to the window boundary, representable time and remaining budget at each level all cap the grant. Its full duration is reserved once against the selected path.

| Slots in first / second window | Parent → child → leaf | Length | Parent / child / leaf balance after reservation |
|---|---|---:|---|
| 0 / 6 | 0 → 1 → 1 | 1 | 2 / 0 / 5 |
| 1–2 / 7–8 | 0 → 2 → 2 | 2 | 0 / 2 / 4 |
| 3–5 / 9–11 | 1 → 3 → 3 | 3 | 3 / 3 / 3 |

Both active leaves under parent 0 stay ready after it exhausts its budget. Leaf 2 still has child and leaf budget. Parent 1 gets the remaining slots until refill restores leaf 1's path. Each awakened leaf repeats a short increment-and-branch loop indefinitely. Its demand exceeds the grant, so transferring the CPU requires an actual timer IRQ. Within a multi-slot grant an IRQ restores the same leaf's saved frame.

Empty levels, invalid references and budgets outside 1 through period invalidate the tree. Premature or zero-length requests, no representable interval and no eligible path return no grant and preserve balances. The guest checks IRQ 30, stack address, saved PC, register and TLS cookies and advancing work. A full missed slot or a dispatch that reaches the following boundary fails the run. Logical slots measure reserved service. Counter intervals reflect dispatch and interrupt overhead. Physical-board bounds, multicore interference and production timing remain open.

## First-launch capture and replay

Build inside kernel with `cargo build --release --locked --features hierarchy-bench -j1`. On Linux create an executable wrapper that forwards its arguments with `exec python3 -B /absolute/path/to/tools/hierarchy_capture.py "$@"`. From the repository root run the following with a new capture directory and the wrapper's actual path.

~~~sh
HIERARCHY_CAPTURE_DIR="$PWD/hierarchy-first" python3 tools/qemu_boot.py \
  --profile hierarchy --qemu /absolute/path/to/wrapper \
  --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
~~~

Before launch record the source revision and uncommitted changes, ELF SHA-256, build command and tool versions. Preserve the first attempt's observer output and exit status with launch.json, uart.bin, int.trace, qmp.json, audit.bin and exit.json, including incomplete attempts. Expected UART after the two boot lines is HIERARCHY WAIT, HIERARCHY THROTTLE, HIERARCHY REFILL and HIERARCHY PASS, one per line.

QMP reads the ELF symbol hierarchy_witness from physical memory. Decode 1,584 bytes as little-endian u64 words. The six header words are completed slots (12), counts per slot (CNTFRQ / 20), first absolute compare value, initial wait mask (15), IRQ count (13) and pass flag (1). Twelve 16-word rows follow.

| Word offsets per row | Fields in order |
|---|---|
| 0–4 | Slot, parent, child, leaf owner, ready mask |
| 5–9 | Grant start slot, grant length, parent balance, child balance, leaf balance |
| 10–12 | Selected PC on entry, interrupted PC on exit, saved frame address |
| 13–15 | Dispatch counter, IRQ observation counter, saved X20 work counter |

Replay each reservation using the table. Its post-reservation balances repeat for all its slots. Check that each stop counter meets its anchored end boundary and precedes the next. Resolve raw PCs against the launched ELF. Each leaf's first selected PC is h_leafN_wake after its SVC. Later selections restore its last interrupted PC in [h_leafN_work, h_leafN_end). Match the four waits, initial idle IRQ, 12 leaf IRQs and exception returns to int.trace. Retain the actual intervals and frame addresses with the raw dump for EVD-024.
