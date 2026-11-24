# Ticket lottery and service-charged stride

[한국어](m4-proportional.md) · [Change #54](https://github.com/Romanof123/JNOX/issues/54)

CHG-023 · POL-010 · REQ-002/005/023 → DES-003/012 → IMP-021 → VER-002/005/023. One Cortex-A72, GICv2, three independently stacked tasks A/B/C with weights 1/2/4. The two lottery epochs each run 32 physical timer grants at a counter rate of 100 grants/s. Use SplitMix64 with initial states 1 and 2 respectively. Reject raw words below 2^64 modulo the eligible ticket total before taking the remainder. With all jobs ready, draws 0, 1–2 and 3–6 select A, B and C. Reject draws outside the active intervals. Per seed each job’s count may differ by at most eight grants from 32 × weight / 7. The independent host calendar uses seeds 1 through 8 and 4,096 draws each, with a per-seed per-job tolerance of three percentage points. These windows test the declared reproducible generator.

Stride chooses the ready task with minimum pass and breaks ties by smaller ID. Subtract the common eligible floor to preserve relative ordering. Charge actual whole residency ticks at 25 ticks/s, multiplied by 720,720 / weight. A yields after one tick. B and C use two. With A/B ready the first eight grants repeat A,B four times and produce service 4,8,0. Introduce C at the incumbent floor. Expect A,B,C,C twice, contributing another 2,4,8 ticks. Record pre-/post-join passes, normalization and post-stop service.

In kernel build `cargo build --release --locked --features proportional-bench -j1`. Run `python3 tools/qemu_boot.py --profile proportional --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel` from the repository root. For the first observation, set PROPORTIONAL_CAPTURE_DIR to a new directory and pass an executable Linux shell wrapper invoking `python3 -B tools/proportional_capture.py "$@"` via --qemu. The first three policy lines are LOTTERY SEED A, LOTTERY SEED B and STRIDE SERVICE JOIN.

The final control runs one more CPU-bound task with a two-tick grant. Keep its IRQ masked past the third service tick, then unmask at prop_delayed_work. Charge the entire observable number of whole ticks, including lateness. Require a PPI30 at that work loop, at least three ticks charged and a pass advance of 720,720 × observed ticks for its weight of 1. Publish STRIDE DELAY CHARGED before PROPORTIONAL PASS. Regular lottery selections have separate count and service accounting. A run outside the specified regular timing window retains the measured row and fails that fixture.

The ELF symbol proportional_witness is 10,432 bytes. Its header has five little-endian u64s (row count, two tick lengths, both seeds) then six u32 passes before and after late admission. Each of 81 128-byte rows begins with fourteen u64s: outgoing ELR, frame, grant start, stop, timer compare, progress, normalization, three passes at selection, three passes after retirement and observed whole ticks. Sixteen bytes follow: epoch, slot, owner, ready mask, interrupt ID, draw, ticket total, interval start, exclusive end, applied flag and six reserved bytes. Value 255 denotes an inapplicable draw or a non-IRQ stop. A completed charge has applied=1. Check all IRQ/SVC entries and saved-frame returns against the frozen ELF. Physical latency, interference and product guarantees remain open.

[EVD-021](evidence/evd-021.en.md)

## First-failure observation (#79)

CHG-037 / IMP-035, POL-010, [EVD-035](evidence/evd-035.en.md). The historical UART ended in PROPORTIONAL FAIL after READY. Four IRQs interrupted B,A,B,A at 0x4020127c, 0x40201260, 0x4020127c, 0x40201260, all inside their ELF work loops. Three handlers returned. Epoch 0 slot 3 is the last interrupted grant. The missing timer, register and predicate data leave the cause unresolved.

With proportional-diagnostic enabled, preserve the first failing source line, zero-based epoch and slot, actual and expected operands before emitting FAIL. A source line refers to proportional.rs from the captured ELF revision. For comparisons read the relation at that line. A boolean or missing policy result uses 0/1. Use the existing symbol-based QMP observer to export the independent failure record on successful and rejected runs. With proportional-fixture-miss explicitly enabled, extend only the first lottery timer grant to two ticks and retain the normal one-tick acceptance criterion. Verify the raw UART fails, QMP exports a committed first record for epoch 0 slot 0 and the measured witness row matches it. A natural earlier failure must remain observable. Archive ordinary and diagnostic results independently.

For Linux collection, build the selected feature in kernel/ and use a fresh directory per invocation:

```sh
PROPORTIONAL_CAPTURE_DIR="$capture" python3 tools/qemu_boot.py --profile proportional --runs 1 --kernel "$elf" --qemu tools/proportional_capture.py
```

Read trace.bin as five little-endian u64 words: source line, epoch, slot, actual, expected. A zero line means no committed failure. Before the first epoch, both coordinates are u64::MAX. The wrapper installs its SIGTERM handler before launching QEMU and forwards UART live. A transcript rejection initiates the same QMP dump as a successful observation. Require launch.json to name proportional_failure at size 40 and verify exit.json, uart.bin and audit.bin independently. Array failures record the first differing pair in iteration order. Preserve the exact source tree and ELF alongside each capture. The historical ELF SHA-256 was 3e771ec6fb331510ffe5f719c3e1eb4840a0c7105af8fd4ce49bed89f132a070.
