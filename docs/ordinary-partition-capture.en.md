# Ordinary Rust partition capture

[한국어](ordinary-partition-capture.md) · [Frozen plan](pol-006-rust-partition-plan.en.md) · [#99](https://github.com/Romanof123/JNOX/issues/99) · [#21](https://github.com/Romanof123/JNOX/issues/21)

CHG-046 affects REQ-002/005/023. IMP-044 partially implements those requirements and POL-006 under DES-003/012. VER-002/005/023 and EVD-044 outcomes remain pending. P and Q are independent cold boots. The workers and idle have distinct Rust entry symbols and private stacks. Only the IRQ handler changes the absolute timer compare. B becomes eligible from the first explicit idle boundary. Select one ordinary table on virt with JNOX_PROFILE and JNOX_APP unset. Other benchmark features and board selections are rejected. Cargo feature `partition-bench` alone continues to select the assembly fixture and its existing entry point.

From a POSIX shell in `kernel`, with the pinned Rust toolchain, QEMU and LLVM utilities installed, run the following commands once for each table. Keep each ELF, UART, disassembly and exception log together. Use a fresh output directory per boot. Stop QEMU after the terminal line. A timeout or partial final line requires an incomplete verdict.

```sh
table=p # repeat with q
out="../capture-ordinary-$table"
mkdir "$out" || exit 1
cargo build --release --features "ordinary-partition-$table" || exit 1
cp target/aarch64-unknown-none/release/jnox-kernel "$out/kernel.elf"
git rev-parse HEAD > "$out/revision"
git write-tree >> "$out/revision"
sha256sum "$out/kernel.elf" > "$out/sha256"
llvm-objdump -d "$out/kernel.elf" > "$out/disassembly"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
  -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio \
  -d int -D "$out/exceptions.log" -kernel "$out/kernel.elf" | tee "$out/uart"
```

All counters are decimal physical counter ticks. 0/1/2 denote A/B/idle, 3 is the boot sentinel. HEADER supplies the epoch and frequency. EDGE records the old compare, IRQ entry sample, live selection sample, pre-eret sample, next compare, and interrupted/selected PC, stack pointer and GPR fingerprint. STACK gives exclusive upper bounds. OBS comes from the running Rust loop after return, including continued execution of the same owner across wrap. Check its SP, id, count and wrapping accumulator independently against ELF symbols and QEMU exception returns. STOP supplies the final outgoing sample. BOUND lists crossed nominal starts; skipped=1 marks a whole slot passed before selection. Reject any trace past the diagnostic horizon of four major frames.

Between consecutive edges, charge the bracket from the worker sample x to its last sample to that task (including measured spill), entry to pre-eret to the dispatcher, and the gaps last-to-entry and pre-eret-to-x to transition uncertainty. Split at every nominal boundary, retain idle separately, and include the boot interval from epoch to first pre-eret. Compare the complete deadline-to-x interval against F/1000; raw entry is sampled after the vector branch and a stack push. Treat ambiguous edge order, missing progress, any skipped runnable window, trace loss and over-limit uncertainty as failed or inconclusive according to the frozen plan. A terminal line alone confers no verdict.

The initial pure-counter probes exercise same-slot and skipped-slot decisions at synthetic counter offsets. The live finite masked case and live just-over-limit case remain open. These probes provide no evidence of measured masking, spilled execution or live recovery. No execution evidence is attached.

## Passive compare evidence and offline accounting

IMP-044 records one TIMER row per returned generation. The guest writes the nominal next boundary d. The counter samples p and after bracket the actual comparator write and register readback c; entry i is in the following EDGE or STOP. Identical c and d are required for healthy guests. Frames P and Q have separate cold-build commands above. No guest delay selector is present. The #106 controlled delayed-notice cases and #99 live masking cases remain open.

The host reconstruction reports disjoint intervals split at each nominal boundary, actual worker/idle ticks, dispatcher ticks, edge uncertainty, lost eligible service, skipped windows, B release eligibility and first subsequent selection. It separates c−d, i−c and x−i at the first A close. The elapsed accounting extends from epoch to the terminal entry sample. JSON origin records the input category declared by the caller. For guest evidence, independently check raw exception trace and ELF as required by the plan. Synthetic rows exercise both tables and all three timing cases and do not provide target execution evidence.

```sh
# From repository root, after collecting a fresh healthy guest UART:
python tools/ordinary_partition_account.py capture-ordinary-p/uart --table P --case healthy --origin guest > guest-P.json
python tools/ordinary_partition_account.py capture-ordinary-q/uart --table Q --case healthy --origin guest > guest-Q.json
# Offline synthetic controls, with no guest:
for table in P Q; do
  for mode in healthy near long; do
    python tools/test_ordinary_partition_account.py --emit "$table" "$mode" > "synthetic-$table-$mode.uart" || exit 1
    python tools/ordinary_partition_account.py "synthetic-$table-$mode.uart" --table "$table" --case "$mode" --origin synthetic > "synthetic-$table-$mode.json" || exit 1
  done
done
python tools/test_ordinary_partition_account.py
```
