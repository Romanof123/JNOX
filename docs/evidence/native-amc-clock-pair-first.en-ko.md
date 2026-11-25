# Native AMC clock pair #353 / #354 · 독립 첫 부팅 검증

**공개 판정 결과는 5건 통과, 1건 실패다.** All four required page-enabled normal-hi/staggered firsts reached HI1's real PPI30 and met their frozen timing bounds. The disabled fallback first also met its contract. The C/Rust compatibility first exercised actual preemption and all time brackets but its public scorer accepts only an IRQ-triggered preemption. The actual scheduling decision occurred at a return gate. Each of the six cold boots reached its terminal record once, with zero interval, lifecycle and evidence-archive drops. The original #348 traced PPI misses remain recorded.

공개 소스 `896518d8edcfc850dc16361dcde2d1edd87c0d4c`에서 문서에 적힌 C·Rust SDK 호출로 네 작업과 대조 앱 두 개를 빌드했다. All public SDK builds, the native C production sampler controls, four deployment packagings and the six-position public seal succeeded on the first invocation. The host controls test the production reader's two attempts, changed/invalid sequence, checked tick conversion/overflow, errors, cached fallback and concurrent unsupported readers. They substitute the host counter operand in the C reader; actual EL0 instructions and mappings were exercised separately in QEMU.

Public prelaunch seal `f1c496b445ba4baf2657abd078d9fda72fe07b26739e6141e026108d5a205e13`. Supplemental seal `76a5d8e0db61f4d9c59805c9dad76c1166f4e6cf15c4a6095c5b2db503574090`. It identifies source, SDK and tools, actual kernel and ordered ELF identities, decoded parameters, independently linked roots/private page backing, QEMU options, timeout of 120 s and per-position capture destinations. The same four ELFs were used for positions 1–5. Position 6 linked the C reader in the low app and Rust public clock_sample in the high app. QEMU virt, Cortex-A72, single CPU, GICv2, 128 MiB and the default TCG clock were used.

| First position | Public verdict | Page / fallback pairs | Clock SVCs | External EL0 IRQ/SVC | b→x (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 normal-hi plain, enabled | observations-met | 842 / 0 | 0 | — | 8.53576 |
| 2 normal-hi traced, enabled | observations-met | 1255 / 0 | 0 | 9 | 7.518816 |
| 3 staggered plain, enabled | observations-met | 1549 / 0 | 0 | — | 7.127168 |
| 4 staggered traced, enabled | observations-met | 1540 / 0 | 0 | 10 | 7.144624 |
| 5 normal-hi traced, disabled | observations-met | 0 / 995 | 1990 | 1999 | 18.683712 |
| 6 C / Rust bracket control, plain | failed | 663 / 0 | 2652 | — | — |

## 예산 경계 / Budget crossings

The fixed counter was 62.5 MHz and Q was 3,125,000 ticks. All five timed firsts recorded HI1/g0 PPI30. Position 5 permitted either actual winning cause. Every saved task stamp in the enabled runs was matched to its generation's published physical interval and already settled service. Disabled stamps matched actual CPU then monotonic clock SVC records.

| First | Boundary b | Programmed CVAL | PPI i | Commit c | First stored HI1 observation x | HI1 service / spill (ticks) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 10541623 | 10577364 | 10584911 | 10592527 | 11075108 | 3132547 / 7547 |
| 2 | 10224740 | 10256359 | 10264622 | 10269668 | 10694666 | 3133263 / 8263 |
| 3 | 19539169 | 19553399 | 19559096 | 19563900 | 19984617 | 3130697 / 5697 |
| 4 | 19457872 | 19475530 | 19485226 | 19490128 | 19904411 | 3134696 / 9696 |
| 5 | 9990668 | 10662613 | 10664574 | 10668630 | 11158400 | 3126961 / 1961 |

**시계 페이지와 회계.** Independently walked each slot's three-level page tables from its linked TTBR0, matched its isolated physical page with the kernel ELF, checked EL0 read-only (AP=3), UXN/PXN and the privileged, writable identity aliases. The disabled position had no page mapping. Enabled firsts reported CNTKCTL_EL1=1, allowing the physical count and disabling EL0 timer controls. The independent decoder matched published R with the real next return timestamp, physical CVAL with the earlier of calendar/budget expiry, and every charged interval against its generation. Retired sequences were zero with zero reserved bytes. The external exception types and PCs matched on all three traced firsts. Evidence of access rights came from live descriptors and the control register.

**앱 결과.** Normal HI produced partial LO0, the surviving HI job and fresh LO0/g1 8/36 before b6. Both staggered firsts retained HI3's 8/36 checkpoint, completed its 16/72 result before b8 and discarded partially run LO0. Each generation's independent ring sum, observed policy result, app exits, charged ticks, deadlines and remaining margins are in independent.json and the original public reports.

## 대조군 최초 판정 / Compatibility control

The public scorer returned `failed` with `control low task did not span high task service`. Its switch predicate requires cause 1 (timer IRQ) for the low-to-high decision. The prospective compatibility contract requires real higher-priority preemption, while positions 1–4 specify PPI30 explicitly. The following events come from that same position-6 capture.

| Event | Data recovered from RAM |
| --- | --- |
| Low→high | Row 909, low/g0 return gate (cause 3, op 0) at tick 6,697,227. Calendar/CVAL 6,697,142. The rejected return charged zero ticks. Low accumulated 2,020,261 ticks before high/g0 began. |
| High→low | Row 1515 at tick 8,634,531. High exited with status 1 and 1,267,255 charged ticks. The kernel returned to low/g0 at its saved PC, SP and root. Low's CPU accounting excluded high's service. |
| Low and fresh low | Low/g0 exited with status 0 after 4,381,486 service ticks. Its recorded clock pairs span ticks 3,687,569 through 12,214,430, on both sides of the high job. Low/g1's first published return had zero settled service and sequence 2. It made two bracketed reads and returned 8/36. |

All 663 independently decoded bracket records fit both their own CPU and monotonic bounds and their exact publication intervals. The 2,652 syscall clock samples appear in explicit CPU, monotonic, monotonic, CPU order. Bracket widths below are actual extrema in nanoseconds.

| App / generation | Pairs | CPU width min–max ns | Monotonic width min–max ns |
| --- | ---: | --- | --- |
| 0.0 | 510 | 4176–214512 | 5024–203008 |
| 1.0 | 151 | 4752–228992 | 6208–202288 |
| 0.1 | 2 | 6144–9936 | 6624–13584 |

**Instruction brackets.** For enabled normal HI, the linked vector's R sample is at 0x4020343c and ERET at 0x402034b0, 29 instruction addresses apart, including final CVAL arm and release-store publication. The first ordered entry samples are two instruction addresses after both lower-EL sync/IRQ entries (save x2 and ISB, then MRS). The archive contains per-position disassembly and exact addresses. These instruction distances establish placement; hardware execution-time uncertainty and a physical product bound remain unmeasured.
