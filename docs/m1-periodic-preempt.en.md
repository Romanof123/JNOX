# M1 timer preemption under RM and DM

[한국어](m1-periodic-preempt.md) · English

CHG-011 extends POL-004. IMP-009 defines the contract for EVD-009.

| Trace | Acceptance condition |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | Preserve the original twelve IRQs and ten job entries. Append an RM epoch and a DM epoch, each lasting three ticks. At tick 0 only A runs (period 2, relative deadline 2). At tick 1 enqueue interrupted A before newly released B (period 3, relative deadline 1), with equal static priorities. RM immediately resumes A, which finishes before B runs. DM runs B to completion before resuming A. |
| REQ-002 / DES-003 / VER-002 | A guest timer IRQ interrupts A during non-yielding work at tick 1 of each new epoch. DM tick 2 stops the timer. The complete guest produces 18 timer IRQs. |
| REQ-005 / DES-003 / VER-005 | Each worker uses its private stack. A resumes at its exact interrupted PC with its live register values and work progress preserved. Both workers complete through SVC #1 in each new epoch, giving four additional completions and 14 in total. |

## Independent observation

Build with periodic-preempt-bench, which enables periodic-bench. The [scheduling catalog](scheduling-policies.en.md) gives the selection order for combined features. From the repository root in WSL Bash, retain the revision, ELF and first black-box verdict, including failure, before inspecting implementation or trace. The observer starts one fresh single-core Cortex-A72 guest by default.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features periodic-preempt-bench -j1)
export PREEMPT_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$PREEMPT_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$PREEMPT_EVIDENCE/revision"
cat > "$PREEMPT_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$PREEMPT_EVIDENCE/int.log" "$@"
SH
chmod +x "$PREEMPT_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile periodic-preempt --kernel "$PREEMPT_EVIDENCE/kernel.elf" --qemu "$PREEMPT_EVIDENCE/qemu-int" 2>&1 | tee "$PREEMPT_EVIDENCE/runner.log"
```

```text
JNOX M1 BOOT
JNOX M1 READY
RM 0 A
RM 0 B
RM 2 A
RM 3 B
RM 4 A
DM 0 B
DM 0 A
DM 2 A
DM 3 B
DM 4 A
RM A START
RM A CONTINUE
RM B RUN
DM A START
DM B RUN
DM A CONTINUE
PREEMPT PASS
```

Require the exact lines above (LF or CRLF), 0.1 seconds of silence and successful host stop. After recording the verdict, map all 18 IRQs and 14 SVC #1 exceptions from the same guest to its retained ELF. Check both tick-1 interrupted PCs and exception returns, private worker stacks, register and progress checks, live ranking, completion order and the terminal timer stop. Feed host-only negative controls to the observer with a missing continuation, reversed DM order or trailing bytes. Each must fail. Acceptance covers these functional traces in QEMU. Board latency and general schedulability require separate evidence.

[EVD-009](evidence/evd-009.en.md)
