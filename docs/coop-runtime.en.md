# Linked cooperative events

CHG-038 / IMP-036 / planned EVD-036, [#80](https://github.com/Romanof123/JNOX/issues/80) / #21 POL-001. Single-core trusted Rust tasks on virt use the ordinary fixed FIFO kernel and equal priorities. The owner stores a fixed-capacity FIFO and a compile-time sized handler table. A message contains a handler index and a u64 value. Enqueue returns the original message with InvalidHandler or Full. Each turn releases one slot before calling one registered function. Its enqueue-only callback appends behind older work and cannot dispatch recursively. A successful enqueue updates the peak depth. A physical counter sample on each side of the call measures elapsed ticks including interruptions. Record per-handler completed calls, cumulative ticks, maximum ticks and strictly over-budget returns with saturating totals. The owner receives the elapsed time and overrun flag after return and chooses its response. These measurements do not establish a hard execution bound.

For coop-events the owner (slot 1) starts with peer 2 ready and peer 0 delayed. While owner executes, the physical timer wakes peer 0; the owner must retain execution until voluntary yield. Peer 2 then peer 0 must finish before the owner resumes. Validate the timer interrupt, disabled timer before yield, UART order and stopped idle PC. Independently decode the application observation symbol coop_app_observations as little-endian u64s: ABI version, capacity, registered handlers, peak, full rejects, invalid rejects, processed count, order of up to four payloads, owner reactions, timer disabled before yield, peer order encoded in decimal, then two rows of four counters in the order described above (unused row zero). Require capacity two, handlers two, payloads 11/22/33, peak two, one reject of each kind. Handler 1 posts 33 while 22 waits; its budget is one tick and its two calls cause two owner reactions. Handler 0 runs once. For coop-single require capacity one, one handler, payloads 11/33, peak one, one rejection of each kind and two reactions. Read nonzero measured times and verify sums/maxima/count consistency. The owner continues after logging each overrun. Preserve both ELF identities, raw UART, IRQ trace, observation bytes and idle PC. A nonreturning handler has no completion sample.

The ABI ends with four per-completion elapsed tick values in dispatch order. Unused slots are zero. Reconstruct each statistics row from these values and the registration budgets. The IRQ must restore its interrupted PC and the four SVC immediates must be 0/1/1/1.

Run from the repository root in Linux with the pinned Rust toolchain, AArch64 binutils and QEMU.
```sh
out=$(mktemp -d)
rustc --edition=2024 --test tools/coop_service_tests.rs -o "$out/host-tests" && "$out/host-tests"
for app in coop-events coop-single; do
  (cd kernel && JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app" cargo build --locked --release) || exit 1
  elf="$out/$app/aarch64-unknown-none/release/jnox-kernel"
  COOP_CAPTURE_DIR="$out/$app-raw" python3 tools/qemu_boot.py --kernel "$elf" --profile "$app" --qemu tools/coop_runtime_capture.py || exit 1
  python3 tools/coop_runtime_check.py "$app" "$out/$app-raw" || exit 1
done
```
