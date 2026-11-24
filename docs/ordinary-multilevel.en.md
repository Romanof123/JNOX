# Planned acceptance for POL-009 (#21, #89)

[한국어](ordinary-multilevel.md) · [EVD-040](evidence/evd-040.en.md)

CHG-042 / IMP-040 / planned EVD-040. On virt CPU0, JNOX_PROFILE=multilevel selects JNOX_APP=mlq or mlfq. JNOX_MULTILEVEL_TRACE=1 separately enables the bounded RAM witness. Trusted linked Rust entries own separate 16 KiB stacks. Up to four tasks initially join in declaration order. MLQ chooses the largest unsigned class and rotates ties. MLFQ uses three nonzero allotments, preserves consumption across yield and block, demotes on exhaustion, and resets all levels at a configured cumulative owner-service period. Ties retain enqueue order across boost.

Measure every outgoing interval in raw CNTPCT counts. Include exception entry/restore latency and IRQ overshoot on its owner, exclude time spent selecting another task. Program CNTP for the earlier of remaining allotment and next boost. At a delayed boundary settle owner service, then coalesce elapsed boosts. Verify frame, queue, class, remaining, timer and owner service independently. Compare an always-ready low class against two runnable higher peers, and observe a CPU worker, early yields, block/wake and post-boost dispatch with a peer ready.

Returned tasks exit. Empty/all-exited configurations park with timer disabled. All-blocked live sets report stall. Invalid wake returns false. Invalid configuration, counter wrap, overflow, bad vector or full diagnostic buffer stop with failure. Actual product entries, classes, execution demand, latency targets and platform evidence remain external inputs.

## Reproduction on Linux

Use a new output directory for each app and trace setting. Run each command serially. Repeat with app=mlfq.

```sh
app=mlq
out=$(mktemp -d)
(cd kernel && JNOX_BOARD=virt JNOX_PROFILE=multilevel JNOX_APP="$app" CARGO_TARGET_DIR="$out/plain" cargo +1.95.0 build --locked --release -j1)
python3 tools/qemu_boot.py --profile "$app" --kernel "$out/plain/aarch64-unknown-none/release/jnox-kernel"
(cd kernel && JNOX_BOARD=virt JNOX_PROFILE=multilevel JNOX_APP="$app" JNOX_MULTILEVEL_TRACE=1 CARGO_TARGET_DIR="$out/diag" cargo +1.95.0 build --locked --release -j1)
ORDINARY_MULTILEVEL_CAPTURE_DIR="$out/capture" python3 tools/qemu_boot.py --profile "$app" --qemu tools/ordinary_multilevel_capture.py --kernel "$out/diag/aarch64-unknown-none/release/jnox-kernel"
python3 tools/ordinary_multilevel_replay.py "$app" "$out/capture/audit.bin"
```

The diagnostic has 256 rows. Each contains the original incoming frame (all 104 words), selected frame address and PC, ESR or PPI/control, raw start/deadline/stop, service totals and post-boundary ranks and balances. The queue immediately before selection packs task IDs plus one into nibbles, oldest first. Capture retains the interrupt stream, UART and terminal idle PC. Linked workloads use one percent of CNTFRQ as their unit, with MLQ quantum 1, MLFQ allotments 4/6/8 and boost period 16. IRQ observation and SVC delivery can cross the programmed deadline. Excess counts carry through lower feedback levels and repeated bottom-level quanta in bounded work. If several boost epochs pass, one reset occurs before the next dispatch. Kernel selection, trace copying and idle contribute no owner counts.
