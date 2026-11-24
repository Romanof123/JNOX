# M1 periodic job releases under RM and DM

[한국어](m1-periodic.md) · English

CHG-009 defines the IMP-007 acceptance contract and independent execution record EVD-007 for POL-004.

| Trace | Acceptance condition |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | One guest runs rate-monotonic (RM), then deadline-monotonic (DM), each for logical ticks 0–5. Task A has period 2 and relative deadline 2, with releases at 0, 2 and 4. B has period 3 and relative deadline 1, with releases at 0 and 3. Both use the same static priority. At tick 0 enqueue B before A in both epochs. Live RM selection dispatches A first and live DM selection dispatches B first. |
| REQ-002 / DES-003 / VER-002 | Twelve guest physical timer IRQs on successive absolute deadlines drive all ticks including empty ticks. DM tick 5 stops the timer. |
| REQ-005 / DES-003 / VER-005 | Each job emits its entry marker while executing on its task’s private stack and completes through SVC #1. After the runnable jobs complete, the controller resumes at its interrupted PC. |

Worker bodies are finite and keep IRQs masked until completion. The bench checks the hardware clock to reject a job that runs into the next release. Preemption of a running worker by a newly released job is a separate acceptance case.

## Independent observation

A fresh verifier records the revision and first launch verdict, including failure, before inspecting source, disassembly or the exception log. Run once in WSL Bash from the repository root. Feature precedence is CBS, periodic, then legacy benchmarks. The periodic observer defaults to one Cortex-A72 guest with one core, GICv2, TCG and 128 MiB.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features periodic-bench -j1)
export PERIODIC_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$PERIODIC_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$PERIODIC_EVIDENCE/revision"
cat > "$PERIODIC_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$PERIODIC_EVIDENCE/int.log" "$@"
SH
chmod +x "$PERIODIC_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile periodic --kernel "$PERIODIC_EVIDENCE/kernel.elf" --qemu "$PERIODIC_EVIDENCE/qemu-int" 2>&1 | tee "$PERIODIC_EVIDENCE/runner.log"
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
PERIODIC PASS
```

Require the thirteen lines above with LF or CRLF, 0.1 seconds of silence and a successful host stop. Retain the runner log, ELF and exception log from this guest. After recording the first verdict, map all twelve IRQs and ten SVC #1 completions to the linked ELF. Check task entry, stack selection, controller return PCs, timer stop, release generation and live ranking against the trace and linked code. Exercise the observer separately with host-only transcripts containing a missing release, reversed tick-0 dispatch or trailing bytes. Each must fail. This contract covers functional releases and dispatch order in QEMU. Board timing and analytical admission are outside its scope.
