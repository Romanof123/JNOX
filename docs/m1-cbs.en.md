# M1 single-core hard constant-bandwidth servers

[한국어](m1-cbs.md) · English

[EVD-006 independent execution record](evidence/evd-006.en.md)

CHG-008 defines the acceptance contract for IMP-006 and the independent execution record EVD-006. POL-008 enforces budget and replenishment. POL-005 orders eligible servers by absolute deadline.

| Trace | Acceptance condition |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | Release two continuously ready servers at logical tick 0. A has budget/period 1/3 and B has 1/4, with demand extending past the observation. Slots 0 through 4 execute A, B, idle, A, B. At tick 2 both servers have exhausted their budgets. A replenishes at 3 and B at 4. Server deadlines are absolute logical ticks. The guest physical timer targets successive absolute counter deadlines anchored to the initial counter value. If IRQ handling misses an entire slot, the guest must fail the run. |
| REQ-002 / DES-003 / VER-002 | Five hardware timer IRQs mark boundaries 1 through 5. The first four cause the observed transfers without voluntary yield. The last stops the timer and ends observation. |
| REQ-005 / DES-003 / VER-005 | A and B resume at their saved PCs on their private stacks. Both check register, stack and progress witnesses before reporting resumption. |

## Independent observation

A fresh verifier records the revision and first launch outcome before reading implementation source or disassembly, including any failed attempt. From the repository root in WSL Bash, run the following once. CBS takes precedence when benchmark features are combined. The cbs observer defaults to one single-core Cortex-A72 GICv2 guest under TCG with 128 MiB.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features cbs-bench -j1)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
export CBS_EVIDENCE=$(mktemp -d)
cp "$ELF" "$CBS_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$CBS_EVIDENCE/revision"
cat > "$CBS_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$CBS_EVIDENCE/int.log" "$@"
SH
chmod +x "$CBS_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile cbs --kernel "$CBS_EVIDENCE/kernel.elf" --qemu "$CBS_EVIDENCE/qemu-int" 2>&1 | tee "$CBS_EVIDENCE/runner.log"
```

Require exactly these nine newline-terminated UART lines, accepting LF or CRLF, a quiet window of 0.1 s and a successful host stop.

```text
JNOX M1 BOOT
JNOX M1 READY
CBS A FIRST
CBS B FIRST
CBS IDLE
CBS A RESUMED
CBS B RESUMED
CBS TIMER STOP
CBS PASS
```

Retain the ELF hash, arguments, PID, UART and exception trace from this same guest. Map each IRQ's interrupted PC and return target in the retained ELF, including the two saved-PC returns and terminal path. Inspect linked code and source for actual execution of idle and both workers, private stacks, witnesses, timer stop and eligibility decisions. Record discrepancies. Host-only observer controls must reject missing, reordered or extra output. These controls provide no extra guest runs.

## Admission controls

Check admission separately on the host for independent, implicit-deadline periodic reservations on an ideal preemptive single core. Admit 1/3 + 1/4 and the exact boundary 1/2 + 1/2. Refuse overload 2/3 + 1/2, zero budget, zero period and budget exceeding period. Include an admission case in which checked arithmetic cannot represent an intermediate result and require conservative refusal. The emulated case establishes neither board timing nor product safety.
