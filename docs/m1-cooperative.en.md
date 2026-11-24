# M1 cooperative scheduling and event dispatch

[한국어](m1-cooperative.md) · English

CHG-014 (issue #37) defines the acceptance contract for POL-001, implementation IMP-012 and independent evidence EVD-012.

| Trace | Acceptance condition |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | Under cooperative FCFS, A (thread ID 1) runs first. B (thread ID 0) becomes ready during A’s execution. A retains the CPU until it yields. Later, waking blocked B also leaves A running. |
| REQ-002 / DES-003 / VER-002 | Exactly one guest physical timer IRQ releases B. Exception return targets the exact interrupted PC in A. |
| REQ-005 / DES-003 / VER-005 | In order, A yields (SVC #2) to B, B blocks (#3) and A resumes, A wakes B (#4) and resumes itself, A exits (#1) to B, then B exits (#1) to the event dispatcher. Map each exception return to the appropriate task context and verify the private stack and preserved-register witnesses at resumption. |

The event queue holds two entries. Preload 1 then 2 and verify a further enqueue fails while full. After the dispatcher removes 1, handler 1 appends 3. Handler 1 must return before the dispatcher invokes handler 2. Handler 3 runs last. Verify the rejection preserves the queued entries and dispatch never nests.

## Independent observation

From the repository root in WSL Bash, retain the revision and record the first launch verdict, including failure, before inspecting code, disassembly or the exception trace. The observer starts one single-core Cortex-A72 guest on QEMU virt with GICv2, TCG and 128 MiB.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features coop-bench -j1)
export COOP_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$COOP_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$COOP_EVIDENCE/revision"
cat > "$COOP_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$COOP_EVIDENCE/int.log" "$@"
SH
chmod +x "$COOP_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile coop --kernel "$COOP_EVIDENCE/kernel.elf" --qemu "$COOP_EVIDENCE/qemu-int" 2>&1 | tee "$COOP_EVIDENCE/runner.log"
```

```text
JNOX M1 BOOT
JNOX M1 READY
COOP A FIRST
COOP IRQ B READY
COOP A RETAINED
COOP B FIRST
COOP A RESUMED
COOP B WOKEN
COOP B RESUMED
EVENT 1
EVENT 2
EVENT 3
COOP PASS
```

Require all thirteen lines, LF or CRLF, followed by 0.1 seconds of silence and successful host stop. Retain the ELF hash, arguments, PID, UART and exception log from the same launch. Map the IRQ and five SVCs to the retained ELF. Inspect the interrupted and resumed PCs, task frames, wake target, queue operations and handler return path. UART alone cannot establish these transitions. Host-only observer controls should reject missing, reordered and trailing output. EVD-012 covers functional behavior in this guest. Physical board execution and timing bounds require separate evidence.
