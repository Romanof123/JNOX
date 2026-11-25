# Native AMC #348 · 통합 QEMU 첫 결과 / Integrated first QEMU results

공개 소스 `f2b2cd724ff8e3d8e7e403c5cc6a6e819f5f4ff0`에 main `0da79b69df1490a04ba7b422842602d5764ac1a7`이 포함된 것을 Git 조상 검사로 확인했다. Four standalone C ELFs and all six packages built from fresh WSL ext4. GNU readelf identified each linked MMU Space array, and an independent calculation verified every private ASID and root against its package. The case/input/ELF/kernel/root manifest and boot order were sealed before the first of 12 separate cold launches. Both modes used default TCG, a Cortex-A72 and GICv2 on QEMU virt, one CPU, 128 MiB and a 30-second capture window. QEMU version was QEMU emulator version 6.2.0 (Debian 1:6.2+dfsg-2ubuntu6.31).

Freeze SHA-256 `6473f1fe8c3d42925ff803b413e18fdbd13e82829ac069f14003cf556694baf0`. Each captured session hash is in its capture stdout and the index. The frozen frequency was 62.5 MHz, 16 ns per counter tick and Q = 3,125,000 ticks.

공개 판정기는 통과 8건, 미완료 2건, 실패 2건을 기록했다. All 12 first boots reached the terminal record. No case was rerun. An independent decoder verified the raw exception, frame, page-table, accounting and ring records. Two externally traced transitions were first observed at a CPU-clock SVC, with zero PPI30 events in the same HI generation. The required IRQ ordering failed in those runs, even though the application observation occurred before the next boundary. Both overlap controls latched the expected deadline fault and refused the overlapping release. The public scorer then applied its general deadline check to the expected late exit.

| 구성 / case | 실행 / mode | Original score | Interval rows | External EL0 IRQ/SVC | Interval/lifecycle drops |
| --- | --- | --- | ---: | ---: | --- |
| within-lo | plain | observations-met | 265 | — | 0/0 |
| within-lo | trace | observations-met | 253 | 252 | 0/0 |
| normal-hi | plain | observations-met | 2229 | — | 0/0 |
| normal-hi | trace | incomplete | 1699 | 1697 | 0/0 |
| over-hi | plain | observations-met | 2821 | — | 0/0 |
| over-hi | trace | observations-met | 2387 | 2385 | 0/0 |
| staggered | plain | observations-met | 2854 | — | 0/0 |
| staggered | trace | incomplete | 2355 | 2353 | 0/0 |
| refusal | plain | observations-met | 1740 | — | 0/0 |
| refusal | trace | observations-met | 1435 | 1431 | 0/0 |
| overlap | plain | failed | 1192 | — | 0/0 |
| overlap | trace | failed | 966 | 964 | 0/0 |

## 전환과 작업 결과 / Transitions and products

| Case / mode | b | Crossing cause / tick | CVAL | Commit | First stored HI1 observation | x − b (ms) |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| normal-hi-plain | 9966355 | PPI30 / 10714144 | 10710484 | 10722092 | 11228946 | 20.201456 |
| normal-hi-trace | 10859139 | CPU-clock SVC / 11707182 | 11707057 | 11711571 | 12239703 | 22.089024 |
| over-hi-plain | 10373778 | PPI30 / 10993821 | 10989799 | 10998357 | 11525680 | 18.430432 |
| over-hi-trace | 10100913 | PPI30 / 10865608 | 10862836 | 10870164 | 11382440 | 20.504432 |
| staggered-plain | 19892993 | PPI30 / 20659865 | 20657627 | 20663237 | 21172416 | 20.470768 |
| staggered-trace | 19380703 | CPU-clock SVC / 20063943 | 20062798 | 20067778 | 20592983 | 19.396480 |

**Normal HI and staggered traced firsts.** In both, the owner was HI1/generation 0. SP was `0x1010f00` and saved and returned PC were `0x40029c`. Normal HI saved and prepared TTBR0_EL1 `0x20000407e2000` and sampled 125 ticks past CVAL. Staggered saved and prepared `0x20000407e1000` and sampled 1,145 ticks past CVAL. Each returned success from syscall 6, clock ID 2. The first stored postcommit application monotonic/CPU pairs were 176,603,840 / 56,618,944 ns and 321,777,408 / 56,775,184 ns. There was no HI1-owned PPI30 for either entire traced first. The same-process external logs agreed with every recorded EL0 IRQ/SVC cause and saved PC.

**Overlap controls.** LO0/g0 exited with status 0 and 8 samples summing to 36 after its intentionally missed deadline. The original firsts were late by 2,875,851 ticks (plain) and 2,665,910 ticks (trace). Both logged a deadline fault followed by an overlap rejection, preserved the original generation and charged it through its real exit. Preserve the scorer’s failed verdict and the observed control behavior.

| Workload | Mode | Specific application product / service outcome | Observed release→terminal (ms) |
| --- | --- | --- | ---: |
| within-lo | plain | HI1 completed inside LO, LO0 8/36 | 26.426560 |
| within-lo | trace | HI1 completed inside LO, LO0 8/36 | 26.378928 |
| normal-hi | plain | Fresh LO0/g1 8/36, survivor completed, HI1 exit after 6888714 service ticks | 1.999760 |
| normal-hi | trace | Fresh LO0/g1 8/36, survivor completed, HI1 exit after 6893311 service ticks | 1.468368 |
| over-hi | plain | Fresh LO0/g1 8/36, survivor completed, HI1 exhaust after 9376569 service ticks | 1.423264 |
| over-hi | trace | Fresh LO0/g1 8/36, survivor completed, HI1 exhaust after 9377953 service ticks | 1.376048 |
| staggered | plain | HI3 checkpoint 8/36, final 16/72 | 175.799040 |
| staggered | trace | HI3 checkpoint 8/36, final 16/72 | 170.348656 |
| refusal | plain | Reset b2 refused (busy), b3 refused (unapproved), b4 accepted | 139.299664 |
| refusal | trace | Reset b2 refused (busy), b3 refused (unapproved), b4 accepted | 133.418960 |
| overlap | plain | LO0 deadline fault, new release refused, old LO0 exited 8/36 | 96.013616 |
| overlap | trace | LO0 deadline fault, new release refused, old LO0 exited 8/36 | 92.654560 |

**Independent evidence.** The standalone decoder used the QMP RAM image, the frozen slot roots, the three levels of AArch64 page tables, UART lifecycle entries and task-owned product data. It checked product hashes through the session inventory and compared final-generation bytes directly with each private VA mapping. For each admitted generation it checked disjoint entry-minus-return physical service, actual terminal service, finite values and ring sums, exit SVC and restored PC/SP. For the traced boots it compared the whole sequence of EL0 exception types and saved PCs against QEMU's external log. The detailed JSON retains per-generation charged ticks, remaining LO/HI allowances at crossing, deadline slack, partial and complete sample counts/sums, QMP mappings, and every observed crossing PPI and app witness.

**Scope.** Times refer to the emulated physical counter. Return timestamps before ERET and incoming exception/vector timestamps bracket guest execution; their instruction-boundary uncertainty has no independent numerical upper bound in this cohort. Recorded CVAL-to-entry spill is a separate observed quantity. Physical-board timing, product FTTI and ASIL approval remain outside these QEMU observations. An initial host invocation encountered a missing pyelftools import before source builds or guest launch. Verification then used the installed pyelftools dependency path. The six frozen first guest runs in each mode are unchanged.
