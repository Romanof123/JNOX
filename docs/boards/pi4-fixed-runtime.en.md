# Linked fixed-priority Rust tasks on Raspberry Pi 4

[한국어](pi4-fixed-runtime.md)

CHG-033 · REQ-002/005/008/016/023 → DES-003/004/012 → IMP-031 → VER-002/005/008/016/023 → EVD-031. [#71](https://github.com/Romanof123/JNOX/issues/71)

Core zero enters at 0x80000. Other cores park before touching shared memory. EL2 startup grants nonsecure EL1 access to the physical counter and timer. A direct EL1 loader must grant this access. The existing BCM2711 GIC path arms PPI 30 with combined EOI and requires secure firmware delegation on physical hardware. The fixed runtime dispatches the same ordinary Rust functions and stack frames as virt. Pi fixed builds admit only uninstrumented FIFO and RR without Cargo features. [Board interrupt contract](../evidence/evd-014.en.md)

Run from the repository root with QEMU set to the raspi4b-capable executable. The packager verifies the pinned firmware listed in boards/pi4/firmware.json.

```sh
set -eu
out=$(mktemp -d)
for app in fixed-fifo fixed-rr; do
  (cd kernel && env -u JNOX_FIXED_TRACE JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app/target" cargo build --release --locked -j1)
  elf="$out/$app/target/aarch64-unknown-none/release/jnox-kernel"
  aarch64-linux-gnu-objcopy -O binary "$elf" "$out/$app/kernel8.img"
  python3 tools/package_pi4.py --kernel "$out/$app/kernel8.img" --output "$out/$app/boot"
  (cd "$out/$app/boot" && sha256sum -c SHA256SUMS)
  sha256sum "$elf" "$out/$app/boot/kernel8.img"
  python3 tools/qemu_boot.py --profile "pi4-$app" --kernel "$out/$app/boot/kernel8.img" --qemu "$QEMU" --runs 2 --capture-dir "$out/$app/capture"
done
```

Require exact LF-only lines, each once, with one second of silence after the final LF. FIFO order is exact. RR also permits HIGH immediately before A START under the published ordering contract.

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED FIFO START
FIXED FIFO A START
FIXED FIFO B YIELD
FIXED FIFO A RESUME
FIXED FIFO HIGH
FIXED FIFO A DONE
FIXED FIFO B SLEEP
FIXED FIFO B DONE
```

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED RR START
FIXED RR A START
FIXED RR HIGH
FIXED RR B START
FIXED RR A DONE
FIXED RR B DONE
```

Retain source revision, raw/ELF hashes, emulator version, raw UART and an exception trace from the staged image. Map IRQ and SVC entries and ERET targets against the same ELF. Identify normal Rust task return through the exit trampoline, higher-priority preemption and RR peer rotation. Measure CNTFRQ_EL0 on raspi4b and use that value for tick conversion. The 12,500,000-tick quantum is 200 ms on virt at 62.5 MHz. Measure host wall time independently. Direct model loading bypasses EEPROM and VideoCore firmware. Two physical power cycles, actual IRQ delegation, board latency and system safety approval remain open under [#34](https://github.com/Romanof123/JNOX/issues/34) and [#21](https://github.com/Romanof123/JNOX/issues/21).

Two cold boots each of FIFO and RR on the Pi 4 model showed high-priority preemption and RR peer rotation. Separate register captures measured a 62.5 MHz counter. Physical boot and timing remain open. [EVD-031](../evidence/evd-031.en.md)

CHG-062 · POL-002 · REQ/VER-002/005/008/016/023 · [Pi 4 physical UART profile plan](pi4-serial-capture.en.md#chg-062)

<a id="chg-063"></a>

## CHG-063 · Pi 4 engineering witness over UART

[#34](https://github.com/Romanof123/JNOX/issues/34) needs attributable on-target observations for POL-002. The current ordinary app stores SP and progress for A/B/HIGH in RAM. Its UART publishes work markers. The optional virt fixed trace publishes up to 64 rows of 19 words, including incoming and selected frame address/ELR/SP, reason, owners, counter and timer state. The separate virt cost journal holds 128 seven-word rows. The current Pi build rejects both options, and the RAM observers rely on a stopped emulator. The missing pieces are a Pi engineering selection, a correlated witness produced by the executing task after dispatch/resume, and a bounded post-terminal export. EVD-031's exception PCs came from the emulator; its ordinary UART bytes carry no IRQ-frame PCs. [Model observations](../evidence/evd-031.en.md)

**Image and recording.** Plan a dedicated build-time witness option for single-core Pi 4 fixed-fifo or fixed-rr only. Reject other boards, apps, benchmark features, and combinations with existing fixed trace/cost. Keep the ordinary build free of witness hooks, buffers and exporter; compare its ELF/loadable digest against the same pinned ordinary reference. Keep the #141 ordinary capture profiles, byte order and final quiet period. Use a separate engineering capture profile and schema. Reuse the fixed trace's bounded CPU-local publication and existing role/progress observations. Record only kernel and linked example-task producers; this scope needs no public tracing ABI or untrusted emitter. [CHG-062 / #141](https://github.com/Romanof123/JNOX/issues/141) · [Application tracing contract](../application-tracing.en.md)

Record a boot/session identifier, selected table, core affinity read from MPIDR, running exception level, measured CNTFRQ, raw counter ticks, schema/role mapping, row capacities, committed count and losses. Keep A/B/HIGH logical roles distinct from FIFO slots 0/1/2 and RR slots 0/2/1. For each relevant IRQ/SVC retain the actual acknowledge or syndrome, active timer state/compare, outgoing role and saved frame/ELR/SP, selected role and frame/ELR/SP, and a dispatch sequence. Publish the generation before restore. At HIGH and first peer entry, and at the first observed continued work of the interrupted task after return, capture that generation, role, linked probe PC, actual SP, counter, and advancing task-owned progress. Use bounded one-shot RAM observations at entry/continuation; retain the previously saved continuation even if the stack frame is later reused. Correlate each sample to the immediately preceding applicable dispatch with no intervening selection. Decode PC ranges and stack slots from the exact ELF and disassembly; the initial ELR may name the entry trampoline. For FIFO's interrupted lower-priority A, its printed A RESUME follows a voluntary yield and precedes HIGH. Require renewed work after HIGH exits and the saved A continuation is selected. For RR show actual peer rotation and later A/B progress. If HIGH runs before observable A work, retain the valid UART order and mark the preemption claim unobserved for that boot.

**Export and acceptance.** Preserve ordinary workload behavior; add no per-event UART, allocation or formatting on the hot path. Record bounded rows then commit a header, rows and integrity-checked footer for export only after all required work, final exit and actual terminal idle entry. Use a stopped/quiesced producer so export cannot recursively generate trace. Report missing terminal, overflow, truncated/reordered rows, unmatched generations, invalid frame/PC, missing positive progress or uncertain chronology as incomplete evidence. Retain first attempts, failure records and raw serial bytes. Bind source/config, raw kernel8.img and ELF hashes, load address, map/disassembly, firmware/config and actual card/package manifest to the capture. Keep board/revision, EEPROM, adapter, witness and full-power-cycle records. Host USB arrival times measure export transport only.

Rehearse ordinary and engineering FIFO/RR separately on raspi4b. Match the engineering rows to an independent exception/vector/ERET log from the same model boot. Exercise parser controls for wrong image, role, generation, missing/extra rows and loss. On P1 repeat each image/profile with two witnessed independent cold starts on the declared board/package. Compare ordinary and engineering functional order and outcomes on each platform. Measure available device-counter intervals and instrumentation overhead on the engineering image; use only genuinely common reference endpoints for paired deltas. Report footprint/stack, observer placement, variance, changed order or unrecoverable deltas. An ordinary UART without counter endpoints cannot provide a numerical uninstrumented service baseline. Causal software evidence supports executed entry and resumed work; saved ELR and selected ELR are kernel observations. Independent physical instruction/ERET history or exact return-to-first-instruction latency needs an appropriate external trace/debug facility. Physical IRQ routing beyond the observed transaction, product bounds and WCET require separate qualification. [REQ-002/005/008/016/023](../requirements.en.md) · [DES-003/004/012](../architecture.en.md) · [VER-002/005/008/016/023](../verification-plan.en.md)

The integrated host collector passed 566 timed checks, eight observer API checks, four simulated-port cases and seven upstream capture tests. [Pi 4 serial capture tool · Host qualification](../evidence/pi4-fixed-serial-host.en.md)

### Engineering image and serial protocol

Set the two engineering selectors only on the Pi 4 fixed FIFO/RR build, with no Cargo features or fixed trace/cost. Assign a distinct nonzero decimal u64 image ID to each immutable source/config/package. Bind it to the SHA-256 of the archived ELF and mounted card's kernel8.img; retain the git revision, build argv and toolchain alongside session.json. The ID identifies the operator's manifest, not a measurement of the running image. Use different IDs for FIFO and RR. Unset both selectors for ordinary builds and compare ordinary ELF/loadable digests independently.

Run the following from repository root. Repeat with fixed-rr and a new ID. Stage the verified package on the operator-identified card before opening the port. PORT, REVISION, EEPROM and CARD_BOOT name the real adapter, board revision, bootloader revision and mounted boot directory. The collector arms each of two witnessed cold boots, stores raw bytes/read timestamps and waits for one second of quiet following the complete footer within the 30-second capture window. Preserve incomplete runs. On raspi4b supply the same packaged image to QEMU with -M raspi4b -accel tcg -display none -serial stdio -kernel and capture -d int -D separately. Use the decoder against that model UART with its own matching manifest. The physical collector's --replay mode tests serial ingestion only.

```sh
set -eu
app=fixed-fifo
id=63001
out=$(mktemp -d)
(cd kernel && env -u JNOX_FIXED_TRACE -u JNOX_FIXED_COST JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP="$app" JNOX_PI4_WITNESS=1 JNOX_PI4_WITNESS_ID="$id" CARGO_TARGET_DIR="$out/target" cargo build --locked --release -j1)
elf="$out/target/aarch64-unknown-none/release/jnox-kernel"
aarch64-linux-gnu-objcopy -O binary "$elf" "$out/kernel8.img"
python3 tools/package_pi4.py --kernel "$out/kernel8.img" --output "$out/boot"
(cd "$out/boot" && sha256sum -c SHA256SUMS)
aarch64-linux-gnu-nm -S -C "$elf" > "$out/symbols"
aarch64-linux-gnu-objdump -d "$elf" > "$out/disassembly"
python3 tools/pi4_fixed_witness_capture.py --profile "pi4-$app-witness" --image-id "$id" --port "$PORT" --board-revision "$REVISION" --eeprom "$EEPROM" --boot-dir "$CARD_BOOT" --elf "$elf" --output "$out/physical"
python3 tools/pi4_fixed_witness.py "$out/physical/run-1/uart.bin" --profile "pi4-$app-witness" --manifest "$out/physical/session.json" --elf "$elf" --image "$CARD_BOOT/kernel8.img"
python3 tools/test_pi4_fixed_witness.py "$out/physical/run-1/uart.bin" "$elf" "pi4-$app-witness" "$id"
```

All PW records end in LF and carry unsigned decimal u64 words. The exporter runs on the selected idle stack after the third actual task exit, with interrupts masked and the timer stopped. Dispatch buffers have 64 entries; each generation reserves two task samples. No event-time formatting takes place. Kind 0 header fields are schema 1, image ID, boot counter sample, mode (FIFO 0/RR 1), raw MPIDR, CurrentEL decoded, CNTFRQ, dispatch/sample capacities, frame bytes, committed dispatches, dispatch losses, and worker/ambiguity losses. Kind 1 gives slot, logical role, stack low/high and linked entry; A/B/HIGH/idle are roles 0/1/2/3 and idle is slot 4. Kind 2 gives generation followed by the 19 fixed-trace words (reason, raw IAR or ESR, counter, outgoing/selected slots, incoming frame/ELR/SP, selected frame/ELR/SP, argument, result, compare enabled, due, post CTL, post CVAL, pre CTL, accounted service), then pre CVAL, CPU, outgoing-boundary A/B progress and selection sample. Incoming fields are zero for boot. The selection sample precedes publication and restore.

Kind 3 gives generation, entry/work kind 0/1, role, raw CPU, linked probe PC, actual probe SP, counter and the task's activation ordinal or published loop count. The work generation is sampled before increment and rechecked after the probe counter read. A preemption across those reads makes the sample ineligible. Kind 4 gives final generation, CPU, actual idle probe PC/SP/counter and terminal entry PC. PW END gives total record count and FNV-1a-64 over every tag and data word as eight little-endian bytes, excluding the footer. This detects transport corruption; SHA-256 digests separately identify artifacts. Exit status 0 requires the decoder's preemption/resume observations, status 2 denotes unobserved behavior in a structurally valid capture, and status 1 denotes invalid data. The decoder also checks expected work-marker order, ELF symbol ranges and slot ownership. Retain source rows and compare each model IRQ/SVC/ERET PC independently. A kernel selection and subsequent advancing task-owned sample bracket resumed work; they do not timestamp the actual ERET or identify the instruction first retired. The reported probe stack usage is a sampled depth, not a maximum.

Both first engineering sessions reached terminal export on the raspi4b model. The shipped decoder rejected both with exit 1 (terminal idle mismatch). [Pi 4 fixed witness · First model rejections](../evidence/pi4-fixed-witness-first.en.md)

## CHG-068 · Correct the host interpretation of the v1 dispatch row

The first raspi4b FIFO and RR captures from #146 retain their original UART, ELF, raw image, manifest, exception trace and host exit 1 (`terminal idle mismatch`). In each tag-2 payload, generation occupies field 0. The incoming frame address, ELR and SP are fields 6–8; the selected frame address, ELR and SP are fields 9–11 (zero-based, excluding the tag). The final SVC exit selects idle and sets its frame's ELR to the linked terminal routine. Tag 4 carries that routine's entry address in field 5. Validate field 10 against it; keep field 9 subject to idle-stack frame bounds and alignment. For post-HIGH continuation compare all three selected fields 9–11 to the originally interrupted fields 6–8. The original decoder used field 9 for the terminal and fields 8–10 for continuation.

Replay both original captures with a separately identified host revision and retain its verdict beside the first rejection. Pin each input's hash and use the recorded ELF, image and manifest; collect no replacement boot. Require controls distinguishing a selected stack address from an executable target, a different valid executable target from the terminal symbol, and a changed continuation or missing causal progress. Keep the existing generation, phase, task, frame, ELF, footer and progress checks. Record a partial result if the required work was unobserved. Physical UART and independently traced hardware execution remain open. [Original #146 protocol and evidence disposition](https://github.com/Romanof123/JNOX/pull/146) · [Producer revision](https://github.com/Romanof123/JNOX/tree/c388505e576f3019966ae2c74215767097c6fc18) [First model evidence](../evidence/pi4-fixed-witness-first.en.md)

[Integrated first boots, ordinary worker traces and engineering verdicts](../evidence/pi4-fixed-witness-integration.en.md)
