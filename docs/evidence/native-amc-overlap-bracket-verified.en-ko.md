# #348 달력 판정기 오프라인 검증 / Offline verification of the deadline decision

**실제 캡처 10건 통과, 기존 전환 2건 미완료.** At reader `bba5355fbc9846a571c95514fdd89e27c59920d1`, both authentic overlap controls pass. All 22 archived falsification cases and four new RAM controls fail as expected. The two formerly accepted controls that moved fault and refusal together by 25 ms now fail the measured calendar decision check.

| Genuine first | b1 / hardware CVAL | RAM row / decision | Lower bound | Actual miss and refusal | Upper bound |
| --- | ---: | --- | ---: | ---: | ---: |
| overlap-plain | 6,879,849 | 609 / PPI30 | 6,883,967 | 6,889,795 | 6,893,979 |
| overlap-trace | 6,597,736 | 513 / return gate | 6,598,969 | 6,602,809 | 6,608,432 |

각 실행에서 이전 행의 소프트웨어 달력이 b1이고 현재 행의 달력이 b1 이후로 바뀐 지점은 하나였다. The bounds are physical-counter ticks from decision entry to the next measured EL0 return. The normal run was driven by PPI30 and the traced run by the return gate. In both, the original LO0/g0 was due at b1, logged the fault and refusal in that decision and eventually exited; the peer remained within its own deadline. The artificially moved timestamps 8,452,295 and 8,165,309 fall outside their corresponding windows.

각 중복 릴리스 기록에서 RAM의 달력 전환을 삭제하거나 전환이 두 번 나타나도록 수정한 네 사례도 모두 거부됐다. Each synthetic case carried its own recomputed session and RAM digests. All genuine and previous synthetic session digests were independently compared with the two retained archives before running the public reporting entry point.

**Normal-HI and staggered first traces still miss the original PPI condition.** Their HI1/g0 CPU-clock SVC entries are 11,707,182 and 20,063,943 ticks, following CVAL 11,707,057 and 20,062,798. The commits occurred at 11,711,571 and 20,067,778. Every EL0 IRQ/SVC cause and saved PC agreed with the external trace, 1,697 and 2,353 events respectively. Neither generation contains a PPI30. The return-gate event in overlap does not provide a crossing IRQ for either workload.

원본 보고서의 최초 판정은 8건 통과·2건 미완료·2건 실패로 보존되어 있다. This run used the archived inputs recorded by parent `837e0e0c`. It did not build or boot a guest. Frozen kernel source was `f2b2cd724ff8e3d8e7e403c5cc6a6e819f5f4ff0`. The original capture archive SHA-256 is `28d9d30d1b7ed7d848118f90cdd5d59291c73fa9934ea1033c7887d52b35e561`. The previous replay archive SHA-256 is `97e286c596e67c0bfa5d025bc11cb5756b69fce5e531665e1e761b44badb0029`.

`raw/verified.json` identifies each authentic and synthetic session, the original and current verdicts, the independently decoded windows, and both PPI checks. Each command, exit status, stdout and stderr is retained.
