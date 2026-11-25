# #348 보존 캡처 재판정 / Retained-capture reanalysis

**수정 판정기에서 실제 캡처는 10건 통과, 2건 미완료다.** Both genuine overlap controls now pass. The scorer still accepts two negative controls in which the recorded fault and refusal have both been moved 25 ms past the actual scheduling decision. The offline verifier therefore records an open defect in the timestamp check.

판정기 `f8988436b1e1de5a86c016895b15d029001dba2d`의 부모 `e0537c0647ece90ecf0311672139eeef25cf354d`에 보존된 원본 압축과 실행별 해시를 확인했다. The original 12 cold boots used kernel source `f2b2cd724ff8e3d8e7e403c5cc6a6e819f5f4ff0`. This analysis used the archived UART, exception traces, memory and package manifests offline. The first report still records 8 observations-met, 2 incomplete and 2 failed. Original archive SHA-256 `28d9d30d1b7ed7d848118f90cdd5d59291c73fa9934ea1033c7887d52b35e561`; first report SHA-256 `2b904814901b03ae5f2f57c29ed99f8983c4ea8ea7717fef0953ff8fa6df8899`.

| Case | Original plain / trace | Rescore plain / trace |
| --- | --- | --- |
| within-lo | observations-met / observations-met | observations-met / observations-met |
| normal-hi | observations-met / incomplete | observations-met / incomplete |
| over-hi | observations-met / observations-met | observations-met / observations-met |
| staggered | observations-met / incomplete | observations-met / incomplete |
| refusal | observations-met / observations-met | observations-met / observations-met |
| overlap | failed / failed | observations-met / observations-met |

## 중복 릴리스 / Overlap control

Both original streams contain LO0/g0 admission, its b1 deadline fault, refusal of the LO0/g1 attempt while g0 remained live, and the eventual real LO0/g0 exit with status 0. The peer exited with status 1 before its deadline. In both, LO0 accumulated zero service at the fault. The independent interval sum equals the recorded terminal charge. The next admitted work never replaced g0; it published 8 samples summing to 36. Only the overlap fixture requires the completed slot-0 generation 0 to be overdue.

| Mode | b1 due | Actual decision entry / cause | Fault and refusal tick | Next dispatch start | LO0 exit | Service ticks | Lateness (ms) |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| plain | 6879849 | 6883967 / PPI30 | 6889795 | 6893979 | 9755700 | 26807 | 46.013616 |
| trace | 6597736 | 6598969 / return gate | 6602809 | 6608432 | 9263646 | 29316 | 42.654560 |

## 반증 시험 / Falsification tests

Each of 11 changes was applied to a separate copy of each overlap capture. Synthetic session digests were recomputed so the scorer evaluated the manipulated observations. The original inventory was rechecked independently. The scorer correctly rejected 20 of 22 cases: another overdue job, a wrong scheduled deadline, a missing fault or refusal, no continuing service, exit before fault, swapped fault and refusal, a shifted fault timestamp alone, a wrong generation, and false service at the fault. Detailed command outputs and inputs are included.

**재현된 판정 누락.** 두 이벤트의 시각을 함께 1,562,500틱(25 ms) 늦추면 잘못된 기록을 두 번 모두 통과시킨다. For plain, the false timestamp is 8,452,295, beyond the actual return to EL0 at 6,893,979. For trace it is 8,165,309, beyond the return at 6,608,432. The genuine interval logs and external trace in the traced case were kept intact. The fault/refusal pair must belong to the first decision servicing b1. The recorded decision can enter via a PPI or the return gate. Comparing only event order, due date and LO0 charged ticks accepts a fault logged during subsequent peer execution.

## 두 전환의 PPI / Crossing evidence

The independent check matched every EL0 IRQ/SVC cause and saved PC with the original external QEMU stream in each incomplete traced case. HI1 had zero PPI30 exceptions across the entire generation. It crossed Q at a successful CPU-clock SVC. The original requirement for an actual crossing PPI before mode commit remains unmet in both; the public scorer continues to return incomplete.

| Traced case | External exceptions | b | CVAL | Crossing SVC | Mode commit | First stored HI1 observation |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| normal-hi-trace | 1697 | 10859139 | 11707057 | 11707182 | 11711571 | 12239703 |
| staggered-trace | 2353 | 19380703 | 20062798 | 20063943 | 20067778 | 20592983 |

First public scores and the actual captures retain their original identity. The archive accompanying this report contains the corrected offline scores, raw command outputs, independent timing checks and synthetic control descriptions. It references the separately sealed original capture archive by hash.
