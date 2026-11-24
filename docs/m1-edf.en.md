# M1 single-core earliest-deadline-first dispatch

[한국어](m1-edf.md) · English

`CHG-006` defines the acceptance contract for `IMP-005`. LOW enters the ready queue first, has higher fixed priority and starts running. A timer releases URGENT with an earlier absolute deadline on the same guest monotonic clock. LOW remains runnable. Fixed priority and FIFO would both keep LOW running.

| Requirement / design / verification | Acceptance condition |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | EDF compares both runnable tasks by absolute deadline and selects URGENT before LOW yields. |
| REQ-002 / DES-003 / VER-002 | The guest timer IRQ interrupts LOW in its work loop and returns directly to URGENT on its own stack. |
| REQ-005 / DES-003 / VER-005 | URGENT finishes via `SVC #1`. Exception return restores LOW’s interrupted PC and stack. LOW checks its saved state and reports completion. |

## Independent observation

Before reviewing source or disassembly, record the candidate revision and observe the first launch using this contract and the candidate ELF. From the repository root in WSL Bash, build once and capture the runner log and exception trace from the same guest.

```sh
(cd kernel && cargo build --release --locked --features edf-bench -j1)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
export EDF_EVIDENCE=$(mktemp -d)
cat > "$EDF_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$EDF_EVIDENCE/int.log" "$@"
SH
chmod +x "$EDF_EVIDENCE/qemu-int"
set -o pipefail
python3 tools/qemu_boot.py --profile edf --kernel "$ELF" --qemu "$EDF_EVIDENCE/qemu-int" 2>&1 | tee "$EDF_EVIDENCE/runner.log"
```

The EDF profile defaults to one fresh single-core Cortex-A72 guest with GICv2, TCG and 128 MiB. Require exactly seven LF- or CRLF-terminated lines and a successful runner exit after the quiet window.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
EDF URGENT RUN
SCHED LOW RESUMED
SCHED PASS
```

For `EVD-005`, retain the ELF SHA-256, launch arguments, PID, UART and same-run exception records. Map the IRQ’s interrupted and return PCs to LOW and URGENT in that ELF. Map the SVC instruction to URGENT and its return PC to LOW’s interrupted instruction. Inspect source and linked code for deadline construction and comparison, timer release, separate stacks, state checks and the thread emitting each marker. Record disagreements and failed first launches.

This case covers single-core emulated EDF dispatch and restoration. Builds including FIFO or RR retain their existing profiles, with RR taking precedence. Product deadline guarantees, worst-case latency, schedulability and multicore behavior require separate evidence.
