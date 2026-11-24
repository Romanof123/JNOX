# Fixed-runtime cost measurement design

[한국어](fixed-cost.md) · English

#90, CHG-043, IMP-041, [EVD-041](evidence/evd-041.en.md). Compare the same pinned FIFO and RR application on one virt CPU with cost disabled/enabled and scheduler trace disabled/enabled. Retain three successful cold boots per cell and all failures. Archive source diff, selectors, tool versions, ELF/hash, nm -S, readelf -lW/-SW, size -A, exact launch arguments, UART, exception log, QMP idle PC and raw dumps. Report file bytes, allocated sections, PT_LOAD filesz/memsz, stack reservation and symbol sizes for each ELF separately. The five 16-KiB stack reservations include an unused slot. Boot stack is outside this study.

Take ordered CNTPCT_EL0 samples (ISB, MRS, ISB) after Rust handler entry, around the complete Fixed::event call (including charging and selection), and after timer programming and frame selection. Record raw endpoints, operation, incoming/returned frame addresses and CNTFRQ_EL0. Compare elapsed ticks and report maxima per operation and same/different frame, retaining sample counts. Nanoseconds = ticks * 1e9 / frequency. Vector entry, assembly save/restore, handler prologue/epilogue and ERET lie outside. Timer acknowledgment, SVC decode and optionally trace publication lie inside the outer interval. Calibration uses the same pair back-to-back at boot. Keep raw inclusive values; one calibration pair cannot remove probe or cache effects. Record insertion after the endpoint perturbs subsequent scheduling. Cost-off images supply footprint and behavioral controls only.

Paint each configured cold stack and idle once before frame fabrication. Clear the fabricated frame to preserve its ordinary zero-initialized semantics. The stopped RAM prefix still equal to 0xA5 bounds the deepest observed changed byte; coincidental pattern values, restored bytes and untouched SP reservations can hide use. Include the initial frame in each result. Require three used task stacks plus idle, intact unused BSS stack, positive frequency, ordered nested samples, nonempty journal, zero overflow, real IRQ switch and exit for each image, FIFO yield/sleep and same-frame return. Require public UART acceptance and one CPU stopped in idle. Missing evidence leaves the case open. No hard timing or stack compliance limit is claimed. These observations do not establish worst-case execution time, physical performance or qualification.

## Capture on Linux or WSL

Select each of fixed-fifo and fixed-rr. Use separate directories for every cell and fresh capture directories per repetition. Unset each disabled selector entirely. Example for cost enabled and trace disabled:

```sh
export JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP=fixed-fifo JNOX_FIXED_COST=1
unset JNOX_FIXED_TRACE
export CARGO_TARGET_DIR="$PWD/out/fifo-cost"
(cd kernel && cargo +1.95.0 build --release --locked -j1)
elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
export FIXED_RUNTIME_CAPTURE_DIR="$PWD/out/fifo-cost-run1"
python3 tools/qemu_boot.py --profile "$JNOX_APP" --qemu tools/fixed_runtime_capture.py --kernel "$elf" --runs 1
python3 tools/fixed_cost_observer.py "$JNOX_APP" "$FIXED_RUNTIME_CAPTURE_DIR"
```

Use the decoder only in cost-enabled cells. Require exactly 89,128 bytes across the two cost symbols (81,920 stack reservation already present in ordinary builds plus 7,208 journal bytes). Verify every PT_LOAD physical range fits the launch RAM from virt base 0x40000000, file size does not exceed memory size, and no overlapping load ranges occur. Preserve all section deltas, including padding and increased code. No additional stack reservations are allowed.
