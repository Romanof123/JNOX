# Pi 4 native SMP archived stage-1 reader

Extract the committed `docs/evidence/pi4-native-smp-first-execution-raw.tar.gz` into a fresh evidence directory. From the repository root, run the following offline command for each requested position. The reader accepts only the five sealed 60475a0 session digests. It writes JSON to stdout.

~~~sh
python3 tools/pi4_native_smp_stage1.py p1 /path/to/sealed/p1 > pair-plain-stage1.json
~~~

| Position | Original launch |
|---|---|
| p1 / p2 | Pair plain / traced |
| p3 / p4 | Remote plain / traced |
| p5 | Swapped plain |

`original_replay` preserves the existing scorer's result. The supplemental evidence identifies ELR, private ASID/root, reached candidate, source and prepared frame digests (102 little-endian u64 words), CPU row, actual return operand, entry and PPI comparator. Use those indices to locate raw frames. The best chain maximizes the worst timestamp margin across both possible orders; A/B in margin names denote the first/second task in the reported order. Strict comparisons require a positive margin. The five stamp margins use measured kernel stamps. The three exception-order margins widen every PPI trap to [max(actual return operand, active comparator), entry sample]. The four continued-user margins bound the first core's return before the second core's inner witness and that witness before its next IRQ, then the converse. A pre-ERET stamp cannot certify when user execution actually resumed. Where only the next EL0-origin exception bounds the completed ERET from above, this test remains inconclusive even with several advancing candidate frames.

Traced reports count possible unlabelled ERET PC assignments, capped at two; source packets carry CPU identity. No cross-core timing is inferred from shared log order. Remote reports preserve startup calibration and separately show any additional lower bound implied by an earlier independent noncoalesced SGI1. The target handoff is excluded from that calibration; the post-send stamp need not precede the receiver. Positive remaining cell-2 margin is required by the original strict deadline.

Exit 0 means a supplemental overlap witness, 2 means overlap remains unobserved, and 1 indicates unavailable or inconsistent inputs. Remote results are separate and must be read even when overlap is unobserved. This tool does not launch an emulator, modify the archive or qualify an additional position.
