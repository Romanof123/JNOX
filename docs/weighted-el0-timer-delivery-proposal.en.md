# Native EL0 timer-delivery measurement proposal

[한국어](weighted-el0-timer-delivery-proposal.md) · [Investigation #184](https://github.com/Romanof123/JNOX/issues/184) · [Parent #177](https://github.com/Romanof123/JNOX/issues/177) · [First capture #183](https://github.com/Romanof123/JNOX/issues/183)

## First share/default result

CHG-088 source `af952e34cea8fbe0aef5c7b4cb1f945a2884fff5` ran two native EL0 workloads with CPU weights A=1 and B=2 on single-vCPU Cortex-A72 QEMU virt/GICv2, default-clock TCG. At 62,500,000 counts/s, Q=625,000 (10 ms), H=5Q/4=781,250 and G=Q/4=156,250. R is the pre-ERET counter sample, C=R+Q the programmed compare, and I the IRQ-vector sample. Service is I−R. All values below are counter ticks.

| Lease | Owner | Previous I to SELECT | SELECT to R | C | I | I−C | I−R | Above H |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 7 | A | 2,333 | 275 | 5,983,313 | 6,166,114 | 182,801 | 807,801 | 26,551 |
| 10 | B | 1,945 | 145 | 8,281,006 | 8,438,812 | 157,806 | 782,806 | 1,556 |

Both STOP records identified physical timer IRQ 30 with timer control 5. Every scheduler gap was ≤43,288. At closure 6, D=2A−B was −2,603, so A had the lower virtual runtime. Its charge increased D to 1,612,999 at closure 7, exceeding 2H by 50,499. At closure 36, total service was 26,615,971 and D was 63,503. The terminal share test passed. Both H overruns and the closure-7 prefix miss remain in the first result. In the separately recorded icount3 share boot, I−C was two ticks at every timer closure.

First session SHA-256 `e57ad789ca11fe342b83778a65b2d2e27fd05876fdc04d8731bbc8aa4762794d`. Stopped RAM SHA-256 `a41a7e09ed9ee63bd476be05e04b66350678b40ab426372790d12d8c06920130`.

## Proposed timing cohort

1. Predeclare separate first cold plain and instrumented default-clock share boots. Record source revisions, ELF and emulator hashes, actual argv, vCPU identity, host scheduling configuration and clock domains. Keep icount3 results in their own cohort.
2. Preserve R, C, I and the full charge. Instrument the exact emulator version to timestamp timer-model expiry, GIC assertion and vCPU interrupt acceptance. Record host off-CPU intervals for the same vCPU thread. Join events by CPU, IRQ, compare and lease order. Bound clock-correlation uncertainty before attributing delay. Where the records cannot distinguish two stages, report their combined interval.
3. Report all 36 closures and the exit tail separately. Each measured timer lease must satisfy Q≤I−R≤H, each gap ≤G and each prefix −H≤D≤2H. At closure 36 require T=A+B≥36Q and |A/T−1/3|≤2H/(108Q). Retain every first outcome, including timing misses. Score the unchanged workload and gates again for any subsequent candidate.

The existing trace establishes compare-to-vector delay. It supplies no separate timestamps for timer expiry, GIC assertion, host scheduling or vCPU acceptance. The proposed cohort tests which interval accounts for the excess in this simulator.
