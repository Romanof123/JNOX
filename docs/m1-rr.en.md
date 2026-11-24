# M1 single-core round-robin scheduling bench

[한국어](m1-rr.md) · English

`CHG-005` defines the acceptance case for `IMP-004`. Two threads, A and B, have the same fixed priority above LOW. A timer must take the CPU away from each peer while it is still executing. The public UART verdict and the exception trace from that same guest must agree.

The independent verifier observed all six exception transitions in one guest. [EVD-004 and host regression results](evidence/evd-004.en.md)

| Trace | Acceptance condition |
| --- | --- |
| `REQ-023` → `DES-012` → `IMP-004` → `VER-023` | Before starting LOW, execute the deterministic policy checks. Emit `POLICY RANK PASS` after the ready-task ranking checks pass and `POLICY SERVICE PASS` after the service and accounting checks pass. |
| `REQ-023`, `REQ-002` → `DES-012`, `DES-003` → `IMP-004` → `VER-023` | The first timer IRQ releases both higher-priority peers. Three subsequent quantum expiries rotate the running peer behind its equal-priority peer in the order A → B → A → B. LOW stays runnable. |
| `REQ-005` → `DES-003` → `IMP-004` → `VER-023` | LOW, A and B have disjoint stacks in the same address space. Each preempted thread continues from its saved PC and stack. The two peers finish once each before LOW resumes. |

The ranking checks exercise fixed priority, FCFS, SJF, SRTF, HRRN, RM, DM, EDF and LLF on supplied ready-task data. The service checks include budget consumption, virtual runtime and ticket selection with a supplied draw. Both markers describe decisions on controlled inputs. The live dispatch workload in this guest uses fixed-priority selection with round robin among peers. [Scheduling policies and full case list](scheduling-policies.en.md) · [Requirements](requirements.en.md) · [Verification plan](verification-plan.en.md)

## Guest sequence

| Current thread | Exception | Expected exception return |
| --- | --- | --- |
| LOW in its work loop | Timer IRQ makes A and B ready | A enters first |
| A in its work loop | Timer IRQ at quantum expiry | B enters first |
| B in its work loop | Timer IRQ at quantum expiry | A resumes at its interrupted PC |
| A running again | Timer IRQ at quantum expiry | B resumes at its interrupted PC |
| B after resumption | `SVC #1` finishes B | A continues from its last interruption |
| A after resumption | `SVC #1` finishes A | LOW resumes at its original interrupted PC |

Only the first IRQ emits `SCHED TIMER IRQ`. A and B must do work on their own stacks without yielding. A thread restart, a cooperative yield or an exception handler printing a peer's markers fails the case. LOW checks its saved state before printing the final two lines.

## One build and one guest

Build once from the repository root with the pinned Rust toolchain and the `aarch64-unknown-none` target. Give the independent verifier the candidate revision, this public contract and the resulting ELF.

```sh
(cd kernel && cargo build --release --locked --features rr-bench -j1)
```

From the candidate root in WSL Bash, capture the first launch. The runner uses the default Linux temporary directory. The wrapper adds QEMU exception logging to the same guest and passes every runner argument through. The `rr` profile defaults to one guest on `virt,gic-version=2`, one Cortex-A72 CPU, TCG and 128 MiB RAM.

```sh
unset TMPDIR TMP TEMP
export RR_EVIDENCE
RR_EVIDENCE=$(mktemp -d)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
cat > "$RR_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$RR_EVIDENCE/int.log" "$@"
SH
chmod +x "$RR_EVIDENCE/qemu-int"
set -o pipefail
python3 tools/qemu_boot.py --profile rr --qemu "$RR_EVIDENCE/qemu-int" --kernel "$ELF" 2>&1 | tee "$RR_EVIDENCE/runner.log"
```

The complete UART stream must contain exactly these twelve lines, each terminated by LF or CRLF.

```text
JNOX M1 BOOT
JNOX M1 READY
POLICY RANK PASS
POLICY SERVICE PASS
SCHED LOW RUN
SCHED TIMER IRQ
RR A FIRST
RR B FIRST
RR A RESUMED
RR B RESUMED
SCHED LOW RESUMED
SCHED PASS
```

Record the first black-box verdict **before** reviewing kernel source, disassembly or the interrupt log. Retain the raw UART, kernel revision, ELF SHA-256, runner command, diagnostics and stop reason. For a complete capture, decode the runner's UART bytes literal with Python's `ast.literal_eval` and verify its length against the reported byte count before saving it. The observer rejects missing, duplicate, reordered or extra bytes, an unterminated final line, a five-second timeout or guest exit before host stop. It checks 0.1 s of silence after the final line and checks the bytes once more at shutdown. Replay swapped first-entry lines, a missing resumption line and the recorded FIFO transcript on the host as negative controls.

## Match the exception trace to the ELF

Use `aarch64-linux-gnu-nm -n -S -C "$ELF"` and `aarch64-linux-gnu-objdump -d -C "$ELF"` on the supplied image. Map each IRQ's saved return address (ELR) to the interrupted thread's work region and each `ERET` target to the entry or saved address shown above. Decode the two SVC immediates and map their call sites to the peer finish paths. Check that the IRQ handler acknowledges the timer through GICv2, handles expiry and completes the interrupt. Check stack bounds and saved-state witnesses for all three threads, and that each finished peer leaves selection. The UART, trace and linked code must support these transitions in one guest.

This case establishes the order of events for the specified workload on one emulated core. A live EDF workload, measured timer frequency or precision, worst-case response time, SMP and physical-board timing each require separate evidence. [Evidence template](templates/verification-evidence.en.md)
