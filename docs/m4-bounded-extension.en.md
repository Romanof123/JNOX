# Bounded scheduling extension for delegated tasks

[한국어](m4-bounded-extension.md) · [Change #59](https://github.com/Romanof123/JNOX/issues/59)

CHG-028 · POL-018 · IMP-026 · planned evidence EVD-026. The optional M4 prototype lets a data program propose which delegated thread runs. On one virtual Cortex-A72, the kernel bounds evaluation, checks the proposal, chooses a fallback and preserves three predeclared windows for an independent thread. The scheduling requirement is REQ-023, with DES-012 and VER-023.

## Decision and timing

The interpreter supports `IfReady`, `Jump`, `Propose` and `Yield`. It accepts at most 32 task records and 16 operations. Each fetched instruction costs one fuel unit, including a backward jump or terminal instruction. The interface caps fuel at 16 and this guest issues five units. The trusted gate checks task ID and generation, readiness, task affinity, the eligible CPU mask and grant length. The grant must fit the current allowance and the time until the next reservation. Fallback grants one tick to the lowest eligible delegated ID on its lowest eligible CPU. An empty eligible set grants zero ticks and dispatches the private idle context.

The EL1 physical counter defines the schedule. For frequency `F`, a slot lasts `Q = floor(F/10)` counter ticks. Slot `i` begins at `S = origin + i*Q`. The evaluator runs with IRQs enabled on a private stack and works on a copy of the policy. Its absolute compare is `S + floor(Q/4)`. It briefly masks IRQs and checks the counter before publishing a completed reply and heartbeat. On expiry the kernel accepts a completed transaction or applies the trusted policy to the unchanged state and observed heartbeat. The selected context timestamps its own first instructions for the slot, with a required time before `S + floor(Q/2)`. Its service ends at the absolute compare `S + Q`. These are nominal 25, 50 and 100 ms boundaries in guest counter time. Evaluation and handler overhead consume part of a delegated slot.

## Nine-slot workload

IDs 0, 1 and 2 have generations 7, 11 and 17. CPU 0 is the sole eligible CPU. Task 0 owns one slot every three slots at offset 2. At those boundaries the kernel bypasses evaluation and selects only the reserved task. Heartbeat progress through slot 4 is 1, 2, 2, 3, 4. It remains 4 for the rest of the run. The watchdog expires at two slots since the last observed increase, including equality.

| Slot | Input | Decision and actual service | VM steps |
|---:|---|---|---:|
| 0 | Ready test, valid proposal for task 2 | Accepted, nominee 2 | 2 |
| 1 | Task 2 requests CPU 1 | Rejected, fallback 1 | 1 |
| 2 | Independent reservation | Reserved 0 | 0 |
| 3 | Task 2 supplies generation 16 | Rejected, fallback 1 | 1 |
| 4 | Backward jump to instruction 0 | Fuel exhausted, fallback 1 | 5 |
| 5 | Independent reservation | Reserved 0 | 0 |
| 6 | Producer loops without publishing | Watchdog expired, fallback 1 | 0 |
| 7 | Producer remains stopped, both delegates unready | Watchdog expired, idle context 3 executes WFI | 0 |
| 8 | Independent reservation | Reserved 0 | 0 |

## Replay and inspect

From the repository root on Linux, build the AArch64 ELF and run one captured boot. The capture helper needs QEMU and the AArch64 GNU tools. Each replay requires a fresh capture directory.

```sh
(cd kernel && cargo build --release --locked --features extension-bench -j1)
elf=kernel/target/aarch64-unknown-none/release/jnox-kernel
run_dir=$(mktemp -d)
export EXTENSION_CAPTURE_DIR="$run_dir/capture"
cat > "$run_dir/qemu" <<'SH'
#!/bin/sh
exec python3 -B tools/extension_capture.py "$@"
SH
chmod +x "$run_dir/qemu"
python3 tools/qemu_boot.py --profile extension --kernel "$elf" --qemu "$run_dir/qemu"
aarch64-linux-gnu-nm -n -S "$elf" > "$run_dir/symbols.txt"
aarch64-linux-gnu-objdump -d "$elf" > "$run_dir/disassembly.txt"
sha256sum "$elf" "$EXTENSION_CAPTURE_DIR"/* > "$run_dir/SHA256SUMS"
```

Retain the source revision, ELF and artifacts from the same boot. The wrapper records UART, QEMU interrupt trace, launch and exit records, QMP CPU and running status, and a physical-memory snapshot in `audit.bin`. The expected UART ends with `EXTENSION EMPTY IDLE`, `EXTENSION RESERVED THREE` and `EXTENSION PASS`. A violated bench invariant prints `EXTENSION FAIL` and parks the CPU. A missing success transcript or inconsistent journal fails the replay.

### Journal ABI

`extension_witness` occupies 1,856 bytes. All fields are little-endian u64. The seven header words are `count, frequency, origin, slot_ticks, eval_ticks, fallback_ticks, watchdog`. After a complete run, `count` is 9. Each row is 200 bytes with the following 25 fields in order.

```text
slot ready_mask proposed_id proposed_generation proposed_cpu
reason steps selected grant progress
eval_cval eval_irq eval_pc eval_frame eval_work
decision_time dispatch_pc first_work service_cval service_irq
service_pc service_frame service_work eval_ppi service_ppi
```

Reason codes used here are 0 accepted, 1 rejected, 2 fuel exhausted, 3 stalled and 7 reserved. Missing proposal ID, proposal CPU, selected task or PPI uses 255. Missing generation and evaluator timing, PC, frame and work fields use zero. A reserved slot has no evaluator IRQ. An idle slot has selected 255 and grant zero, with real idle frame and PC. All other service slots have grant one.

For independent replay, compare the nine decisions with the table and the six evaluator plus nine service interrupts with `int.trace`. Each interrupt must report physical timer PPI 30 with an enabled, unmasked, asserted source at guest entry. Compare values must match the absolute boundaries. In a delegated slot, require `eval_cval <= eval_irq <= decision_time <= first_work < S + fallback_ticks`. For every slot require `service_cval <= service_irq < service_cval + eval_ticks`. Use the matching ELF to map the dispatched entry or saved PC and the later interrupted `service_pc` into the actual context. Repeated dispatch of the same service context restores its previous saved PC and private frame. Work counters advance on that context. The stopped evaluator retains zero loop progress.

The policy, trusted gate, timer/journal code and assembly loops can be inspected together with the host decision tests. [Interpreter](../kernel/src/extension_policy.rs) · [Gate](../kernel/src/group_policy.rs) · [Guest](../kernel/src/extension.rs) · [Task code](../kernel/src/extension.S) · [Host tests](../tools/extension_tests.rs)

The interpreter and trusted gate execute at EL1 in a shared address space. Private stacks, checked data, fuel and time limits provide the containment exercised by this prototype. Product adoption, protection against arbitrary native code, physical worst-case response time, workload qualification, hazard analysis and functional-safety approval remain open.
