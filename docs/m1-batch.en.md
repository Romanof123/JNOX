# CPU burst dispatch on one core

[한국어](m1-batch.md)

CHG-015 · POL-003 · REQ-002/005/023 → DES-003/012 → IMP-013 → VER-002/005/023 → EVD-013 · [#39](https://github.com/Romanof123/JNOX/issues/39)

A reusable bounded scheduler accounts CPU service to its incumbent in monotonic logical ticks. Releases enqueue a burst and completion explicitly retires it. A separate decision sees all releases at that instant. SJF and HRRN run the incumbent to completion. SRTF reevaluates remaining time on arrival and retains the incumbent on a tie. Equal waiting ranks keep queue order. When preempted, an incumbent precedes equal waiting peers. An unfinished burst has a minimum remaining estimate of one even if it has overrun. Invalid operations preserve clock, jobs and queue.

| Policy | Workload and required dispatch |
| --- | --- |
| SJF | A starts at 0 with estimate 5. A timer at logical tick 1 releases B with estimate 2. A has 4 left and continues to completion at 5. B completes at 7. |
| SRTF | The same arrival preempts A. B completes at 3 and returns directly to the exact interrupted instruction in A. A completes at 7. |
| HRRN | A starts at 0, B (estimate 5) waits from 0. A runs past its estimate of 2. An IRQ releases C (estimate 1) at 10. A returns to its interrupted work and completes. B scores 3 and C scores 1. B runs next, then C. |

Build from the kernel directory with `cargo build --release --locked --features batch-bench -j1`. Keep the revision, ELF digest and QEMU version. Before seeing implementation or trace, run the public observer once from the repository root with `python3 tools/qemu_boot.py --profile batch --kernel "$ELF"`. To retain same-guest exception evidence, pass `--qemu` a wrapper that executes QEMU with `-d int -D "$TRACE"`. Record the first verdict, raw UART and process status before examining the original trace and ELF. Require the lines below, 0.1 s quiet, three physical-timer IRQs and seven SVC #1 exits. Map each IRQ PC/return and each job completion. Corroborate stack and resumption witnesses. Replay missing, reordered and extra UART bytes on the host.

```text
JNOX M1 BOOT
JNOX M1 READY
BATCH SJF A FIRST
BATCH SJF IRQ B READY
BATCH SJF A RETAINED
BATCH SJF B FIRST
BATCH SRTF A FIRST
BATCH SRTF IRQ B READY
BATCH SRTF B FIRST
BATCH SRTF A RESUMED
BATCH HRRN A FIRST
BATCH HRRN IRQ C READY
BATCH HRRN A RETAINED
BATCH HRRN B FIRST
BATCH HRRN C FIRST
BATCH PASS
```

Compare all three policies to an independent small workload oracle. Include tied estimates, a runnable underestimate, zero capacity, invalid and duplicate releases, invalid completions, clock regression, ID reuse and the maximum representable timestamp. Dispatch times in this fixture are model inputs. The three real IRQs establish asynchronous arrival and actual context selection. Hardware WCET, physical timing and product approval remain separate cases.

[EVD-013](evidence/evd-013.en.md)

POL-003 / IMP-037. Six ordinary boots and three diagnostic boots produced 60 independently replayed scheduling decisions across SJF, SRTF and HRRN. [EVD-037](evidence/evd-037.en.md)
