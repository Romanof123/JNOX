# Ordinary Rust partition capture

[한국어](ordinary-partition-capture.md) · [Frozen plan](pol-006-rust-partition-plan.en.md) · [#99](https://github.com/Romanof123/JNOX/issues/99) · [#21](https://github.com/Romanof123/JNOX/issues/21)

CHG-046 affects REQ-002/005/023. IMP-044 partially implements those requirements and POL-006 under DES-003/012.  P and Q are independent cold boots. The workers and idle have distinct Rust entry symbols and private stacks. The kernel programs the first absolute compare at startup and subsequent compares in its timer IRQ handler. B becomes eligible from the first explicit idle boundary. With JNOX_PROFILE and JNOX_APP unset on virt, the default builds the boot image. Either `ordinary-partition-p` or `ordinary-partition-q` alone selects that table. Both together without a legacy benchmark are ambiguous and rejected. Ordinary tables require virt. Whenever legacy benchmark features are present, their original precedence selects the entry point. For example, `partition-bench,ordinary-partition-q` selects the assembly fixture, `cbs-bench,ordinary-partition-p` selects CBS, and `--all-features` selects the same cooperative entry as `coop-bench`. Legacy board/profile restrictions still apply. From the repository root, run `python tools/test_partition_selectors.py` to check these selectors.

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

The initial pure-counter probes exercise same-slot and skipped-slot decisions at synthetic counter offsets. The live finite worker-masking case remains open. These probes provide no evidence of measured masking, spilled execution or live recovery.

## Passive compare evidence and offline accounting

IMP-044 records one TIMER row per returned generation. A healthy guest writes the nominal next boundary d. The counter samples p and after bracket the comparator write and register readback c. Entry i is in the following EDGE or STOP. Healthy guests require c=d. The commands above run healthy P and Q with `JNOX_PARTITION_NOTICE` unset.

The host reconstruction reports disjoint intervals split at each nominal boundary, actual worker/idle ticks, dispatcher ticks, edge uncertainty, lost eligible service, skipped windows, B release eligibility and first subsequent selection. It separates c−d, i−c and x−i at the first A close. The elapsed accounting extends from epoch to the terminal entry sample. JSON origin records the input category declared by the caller. For guest evidence, independently check raw exception trace and ELF as required by the plan. Synthetic rows exercise both tables and all three timing cases and do not provide target execution evidence.

```sh
# From repository root, after collecting a fresh healthy guest UART:
python tools/ordinary_partition_account.py capture-ordinary-p/uart --table P --case healthy --origin guest --elf capture-ordinary-p/kernel.elf --elf-sha256 "$(cut -d' ' -f1 capture-ordinary-p/sha256)" --exceptions capture-ordinary-p/exceptions.log > guest-P.json
python tools/ordinary_partition_account.py capture-ordinary-q/uart --table Q --case healthy --origin guest --elf capture-ordinary-q/kernel.elf --elf-sha256 "$(cut -d' ' -f1 capture-ordinary-q/sha256)" --exceptions capture-ordinary-q/exceptions.log > guest-Q.json
# Offline synthetic controls, with no guest:
for table in P Q; do
  for mode in healthy near long; do
    python tools/test_ordinary_partition_account.py --emit "$table" "$mode" > "synthetic-$table-$mode.uart" || exit 1
    python tools/ordinary_partition_account.py "synthetic-$table-$mode.uart" --table "$table" --case "$mode" --origin synthetic > "synthetic-$table-$mode.json" || exit 1
  done
done
python tools/test_ordinary_partition_account.py
```

For guest inputs, supply the original ELF, its expected SHA-256 from the independent boot record, and the raw QEMU exception log. The tool requires llvm-nm or aarch64-linux-gnu-nm. It checks initial PC and SP against the ELF, interrupted PCs against each worker’s symbol extent, private stack ranges, and ordered exception returns and IRQ ELRs. Unknown symbol layouts and missing evidence yield no verdict. Each observed accumulator is independently recomputed from its worker seed using the measured iteration count, with a cumulative limit of 64 million iterations. Synthetic traces use a fixed fictitious symbol map and carry no ELF or exception verification.

The three B fields use raw counter ticks. b_release_nominal is the eligibility boundary. b_first_selection is the dispatcher observation that actually selects B. b_first_worker_observation is the subsequent observation made by B itself. Completed healthy traces retain over-limit edges and skipped slots in JSON and return status 2 on timing failure. Status 1 indicates an invalid or incomplete input. These offline checks preserve the first P and Q measurements and do not improve latency.

Generate current synthetic controls with the commands above. The six original emitted streams in v1 are expected rejections under this checker.

POL-006 / IMP-044. The fresh healthy boots met 1 ms at all 10 P and 13 Q edges. In v1, 9/10 P edges and 11/12 Q edges exceeded 1 ms and Q skipped a runnable B window. All four delayed virtual timer notifications exceeded the original 1 ms envelope at injection and recovered to the specified window. No unrelated edge exceeded the limit. Live worker masking remains unverified. [EVD-044 · delayed notice](evidence/evd-044-notice.en.md) [EVD-044 v2](evidence/evd-044-v2.en.md)

All four delayed virtual timer notifications exceeded the original 1 ms envelope at injection and recovered to the specified window. No unrelated edge exceeded the limit. Live worker masking remains unverified.

POL-006 / IMP-044. Both first ordinary runs failed the 1 ms criterion. Q skipped an eligible B window. Live masking and complete release qualification remain open. [EVD-044](evidence/evd-044.en.md)

## Controlled timer notification on virt

`JNOX_PARTITION_NOTICE=short` or `long` selects one cold test together with exactly one effective ordinary P/Q table. The boot NOTICE and the linked entry symbol identify the same case. Only the initial A window receives the later compare. Short uses c=d+F/1000+1 and long uses c=d+5q/4. All EDGE due/next and TIMER d values retain the original schedule. Both workers keep their original IRQ-enabled loops and private stacks.

From the repository root in a Linux shell with the pinned Rust toolchain, QEMU, GNU timeout, and LLVM or AArch64 GNU objdump and nm, these commands build and capture each case once. For an independently supplied ELF, invoke the capture script directly with its producer's SHA-256 and a new output directory. Preserve the build revision alongside that digest. The recorder identifies its own checkout in checker-revision.

```sh
set -eu
unset JNOX_PROFILE JNOX_APP
out=$(mktemp -d)
printf '%s\n' "$out"
for table in P Q; do
  feature=$(printf '%s' "$table" | tr 'PQ' 'pq')
  for mode in short long; do
    build="$out/build-$table-$mode"
    (cd kernel && JNOX_BOARD=virt JNOX_PARTITION_NOTICE="$mode" \
      cargo build --release --locked -j 1 --target-dir "$build" --features "ordinary-partition-$feature")
    elf="$build/aarch64-unknown-none/release/jnox-kernel"
    sha=$(sha256sum "$elf" | cut -d' ' -f1)
    status=0
    sh tools/capture_partition_notice.sh "$table" "$mode" "$elf" "$sha" "$out/$table-$mode" || status=$?
    printf '%s %s %s\n' "$table" "$mode" "$status"
  done
done
```

The host stops each parked guest after eight seconds and retains UART, QEMU diagnostics, exception trace, ELF, disassembly and first status. Each QEMU output file has a 1,024-block limit. A complete STOP and the final independent IRQ record are required. The offline CLI accepts `--case short` and its existing spelling `near`. Use `--origin guest` with the captured ELF, SHA-256 and exceptions.log.

RELEASE brackets the actual change to B's ready state between before and after at generation n, separately from the scheduled due time. Each IRQ row records the acknowledged interrupt, active compare readback, timer control and interrupted SPSR before the handler disables the timer, and the exclusive end tick computed from IRQ entry. The checker matches EL1 IRQ vector targets and return PCs to the ELF, including the terminal IRQ. It requires two distinct samples made by A after entering the recovery window and splits known outgoing A time through its last sample at every nominal boundary. The remaining interval up to the IRQ entry sample stays in transition uncertainty.

Each result identifies the lost service recipient in each affected window, the actual recovered context and its observed work in the remainder, the next two protected windows, and the original three-frame horizon. P long loses idle service in both the skipped blocked-B slot and the following explicit idle. Q long loses eligible B service on both sides of wrap. The short cases have no skipped slot. A case passes with a measured original timing failure at the injected close, the specified recovery, and all other scored edges within the original bound. Status 2 preserves a reconstructed capture with unrelated timing failures. Status 1 reports invalid or incomplete evidence. The earlier healthy and synthetic inputs keep their existing formats. Separate live worker masking and physical platform qualification remain open. [Controlled-case plan](https://github.com/Romanof123/JNOX/pull/106)
