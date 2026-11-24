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

## IMP-046 boundary workloads

Select one row per fresh QEMU virt boot. For all four use `JNOX_BOARD=virt JNOX_PROFILE=multilevel`. The plain selector leaves `JNOX_MULTILEVEL_TRACE` unset. The diagnostic selector adds `JNOX_MULTILEVEL_TRACE=1`.

| Plain JNOX_APP | Diagnostic JNOX_APP | Workload |
| --- | --- | --- |
| mlq-blocked | mlq-blocked | Two equal-class entries call block; STALL |
| mlq-exited | mlq-exited | Two equal-class entries return; DONE |
| mlq-saturated | mlq-saturated | High peers share an absolute end at initial start + 24U; low works for U |
| mlfq-delayed | mlfq-delayed | Three CPU workers share initial start + 96U; first comparator uses start + 3P + P/4 |

Use the build commands above with each selector. For *both* variants run the capture wrapper command above using a separate new capture directory and the corresponding ELF. The default UART deadline is five host-monotonic seconds and all four profiles require one further second of quiet. The plain capture writes the eight-byte post-disable CNTP_CTL_EL0 readback to audit.bin. Diagnostic capture writes that readback separately to multilevel_terminal_ctl.bin and dumps the complete journal to audit.bin. Both retain the QEMU exception trace and terminal PC.

The EVD-040 journal prefix is byte-compatible. Each new diagnostic app appends 256 rows of 11 little-endian u64 words after the entire original 256-row array. Index both arrays with the same decision number.

| Word | Observation |
| --- | --- |
| 0–2 | Queue before boost, outgoing rank and balance after full charge and before reset. Zero on initial row |
| 3 | Cumulative calls that executed the policy reset |
| 4–5 | Live and asleep slot bitmaps (bit i for slot i, unused slots asleep) |
| 6–7 | Outgoing comparator and control read back immediately after arming. Zero on initial row |
| 8 | Control read back after terminal disable, otherwise UINT64_MAX |
| 9 | Absolute workload end in counter units, zero in terminal-state workloads |
| 10 | Total owner service before outgoing charge |

The independent observer must compare actual comparator and timing against the nominal deadline in the prefix, including the first ordinary timer after the deliberate delay. Preserve each first capture and verify the finite saturation window, per-owner charges, intermediate balances, queue order and later epoch crossing against the [frozen plan](multilevel-limits-plan.md). Historical replay for the original two selectors uses their original layout.

POL-009 / IMP-046. Six finite workloads passed in ordinary and diagnostic builds. [EVD-046](evidence/evd-046.en.md)
