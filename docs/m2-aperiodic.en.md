# Aperiodic request service in reserved CPU intervals

[한국어](m2-aperiodic.md)

CHG-018 · POL-007 · REQ-023/002/005 → DES-012/003 → IMP-016 → VER-023/002/005 → EVD-016. [#44](https://github.com/Romanof123/JNOX/issues/44)

Use the same FIFO workload with budget 2 and period 4 under polling (PS), deferrable (DS) and sporadic (SS). SS stores two pending credits and SS1 stores one. Unused polling capacity is lost when the backlog drains. Deferrable service retains remaining capacity until the period boundary. Sporadic service returns consumed units one period after their slot. On credit-queue saturation, merge the last credit and the new one at the later deadline.

At ticks 1, 3, 4 and 7 the producer submits (ID, units) = (1,1), (2,3), (3,1), (4,1) via SVC #1. It checks the acknowledgement and parks via SVC #2 before the grant. Each successful grant reserves one unit and selects the work context. The corresponding request remains outstanding until the next timer IRQ witnesses execution on that context and retires the unit through its receipt. The receipt binds the instance, tick and ID. Allocate a distinct instance key while receipts from a previous lifetime can survive. An unserved interval runs idle.

The table gives the request ID granted in slots 0–9 and final queued units. ID 0 denotes idle in this fixture. The observer also requires the complete per-slot remaining budget and credit list, from its published byte sequence. Each row sends six work units. The charged and completed counts in the final record count units.

| Server | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | Pending |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| PS | 0 | 0 | 0 | 0 | 1 | 2 | 0 | 0 | 2 | 2 | 2 |
| DS | 0 | 1 | 0 | 2 | 2 | 2 | 0 | 0 | 3 | 4 | 0 |
| SS | 0 | 1 | 0 | 2 | 0 | 2 | 0 | 2 | 0 | 3 | 1 |
| SS1 | 0 | 1 | 0 | 2 | 0 | 0 | 0 | 2 | 2 | 0 | 2 |

From kernel/ build `cargo build --release --locked --features aperiodic-bench -j1`. From the repository root run `python3 tools/qemu_boot.py --profile aperiodic --kernel "$ELF"`. A fresh verifier records revision, ELF hash, first verdict, exit status and full UART before opening code or same-run trace. Capture `-d int` via a QEMU wrapper. In one guest map 40 physical timer IRQs and 32 producer SVCs (16 pairs), including actual interrupted worker/idle loops, checked producer acknowledgements, saved-context returns and shutdown. Real one-shot deadlines are anchored to each epoch at CNTFRQ/20. A full missed period fails the run. Verify the quiet window and reject missing, altered-credit and trailing UART data.

A separate host model varies arrivals and CPU availability and checks accounting before and after completion. Include full capacity, invalid submissions, same-ID reuse, stale/foreign receipts, clock/work overflow and credit coalescing. Hardware interference, product deadlines, general service overload handling and physical memory isolation remain separate work.

[EVD-016](evidence/evd-016.en.md)

CHG-047 · [Ordinary Rust server workload and admission criteria](pol-007-rust-server-plan.md#english) · [#101](https://github.com/Romanof123/JNOX/issues/101)

POL-007 / IMP-045. The standalone default series stopped 10/10 before arbitration. Integrated default and icount each completed 8/8 ordinary cases and isolated 2/2 late controls with matching ELF hashes. Timing variation remains unexplained. [EVD-045](evidence/evd-045.en.md)
