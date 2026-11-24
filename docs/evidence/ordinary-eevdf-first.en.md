# CHG-083 · First ordinary EEVDF telemetry boot

The first cold QEMU virt boot of e203caa997eea8a561c89e59fb41994b64547214 qualified for the declared mix. At 62.5 MHz, Q was 625,000 ticks. Independent accounting joined 84 intervals to 84 same-process ERETs, 73 IRQs and 11 SVCs. Both batch clients resumed their interrupted work. The last exit left no runnable owner. Total scheduler gaps were 145958 ticks.

| Window offset | Delivered ticks | A error | B error | C error |
|---|---:|---:|---:|---:|
| 0Q | 19926097 | -71485/3 | 1068077/6 | -308369/2 |
| 8Q | 19939798 | 3765917/6 | -3339031/12 | -1397601/4 |
| 16Q | 19944019 | 1546451/6 | 280064/3 | -702193/2 |
| 24Q | 19935343 | 8011073/15 | -429347/15 | -2527242/5 |
| 32Q | 19940497 | 20637151/30 | -695327/15 | -6415499/10 |

All five 32Q windows delivered at least 28Q. All 15 signed actual-minus-ideal values stayed within ±4Q. The fifth accepted wait at row 41 changed B’s weight from 1 to 2 and request from Q to 2Q before row 42. The A+B reference was 6,877,729; the rounded lag residual was zero. The eight due-to-first delays had nearest-rank p50/p95/max of 742,809/817,052/817,052 ticks; wake-to-first was 736,012/811,904/811,904. Batch A returned (125620416,2991336156,11965344778), B (91242048,2172701268,8690804967), and C (128,27028,115128). Independent ring arithmetic reproduced all three.

| Sequence | Due | Wake | First task sample | Due-to-first | Wake-to-first |
|---:|---:|---:|---:|---:|---:|
| 1 | 5085958 | 5097663 | 5721542 | 635584 | 623879 |
| 2 | 10085958 | 10091531 | 10093654 | 7696 | 2123 |
| 3 | 15085958 | 15092802 | 15185577 | 99619 | 92775 |
| 4 | 20085958 | 20092755 | 20828767 | 742809 | 736012 |
| 5 | 25085958 | 25091220 | 25859267 | 773309 | 768047 |
| 6 | 30085958 | 30097054 | 30872163 | 786205 | 775109 |
| 7 | 35085958 | 35093038 | 35890927 | 804969 | 797889 |
| 8 | 40085958 | 40091106 | 40903010 | 817052 | 811904 |

Five historical EEVDF host tests passed, five incompatible configurations were rejected, and zero fairness tolerance, zero response tolerance and missing response controls failed as expected. Against parent 592358d, all three legacy benchmark builds retained identical executable text; one loaded diagnostic source line changed from 179 to 204 in each image. Those images were not booted. Results cover a finite single-vCPU mix; board timing, sustained admission and M5 remain open.

[JSON](ordinary-eevdf-first.json) · [ZIP](ordinary-eevdf-first.zip)
