# Native batch offline verdict / 네이티브 배치 오프라인 판정

At public 5bcd098d894540cc870d1cdd7eb5906bd015fe2c we extracted the Git archives at 7d0cfb08, e707ca1 and 70f7cf42, verified archive SHA-256s in archive-pins, verified both observer file inventories, and replayed the original 14 native sessions and four observer sessions without launching a guest.

| #341 position | Result from preserved firsts |
|---|---|
| Arrival SJF / HRRN, plain + traced (4) | Productive A kept the CPU at B release; A17 then B43. |
| Arrival SRTF, plain + traced (2) | Lower-EL PPI30 at 0x400808; B43 then resumed A17. Original saved x0–x30 decode. W21 implies 13721 / 13619 completed blocks. |
| HRRN aging / SJF control (2) | After A exit, HRRN chose older B and SJF chose C. |
| SRTF underestimate (1) | Both remaining values at one, A retained. |
| SJF / SRTF / HRRN queued ties (3) | All completed A/B/C in input order. SJF ordinary first admitted all three together. Distinct supplemental SJF observations measured running-incumbent retention. |
| Lifecycle / delayed solo (2) | Yield, sleep, idle/wake and completion replayed with original private results; delayed solo B produced 2048 and exited 43. |

Prior observer SRTF exit 2, original e707ca1: directly measured 13158 blocks at preemption, no independent restored operand; terminal 32768. Prior SJF exit 2: direct clock SVC retained A, 2235 publications inferred before release, subsequent terminal 7647. The simultaneous and immediately subsequent private readings remain absent in that capture.

Corrected SRTF exit 0. At the lower-EL PPI30 A's block/sum words were 8863/4628613120. Distinct save and accepted post-B restore samples at PA 0x40ab4460 had identical x0–x30 and PC/SP/PSTATE. External ERET returned to 0x400808; the first later measured private publication was 8864/4629135360, terminal 32768/17112760320. This proves the values of memory operands consumed by the restore.

Corrected SJF exit 0. Actual release row 2 had kind 3/op 0, timer compare 12869237, entry 12869339, decision 12874281 and EL0 return boundary 12894522. The EL1 vector checks its counter then calls native_batch_recheck on the retained clock-SVC frame. Queue B,C joined A; A's unchanged PC 0x40029c, SP, PSTATE, root and frame identify the returning incumbent. Private block/sum words were 2148/1121771520 then 2149/1122293760. Setup plus 2148 loop clocks gives ordinal 2149, already included in transparent calls. The contiguous external A stream covers 7773 clock returns before A exit; the EL1 recheck creates no separate exception. Terminal A 7772/4058849280. Exits A17/B43/C17. This classifies the same corrected capture that previously returned 2.

Nine independently rehashed offline tamper controls gave zero false passes. Wrong exception kind, decision PA, compensated ordinal, private word and restored x23 returned 1. Uncompensated ordinal, inverted decision/return chronology, altered retained PC, and corrupted external ERET returned 2. See challenge.py/log. Memory-backed observer positions used 128 MiB shared file RAM and callback timing. QEMU evidence supplies architectural return and operands; physical-board timing and live register-file sampling remain separate.

원래 14개 배치 실행을 모두 오프라인으로 다시 판독했다. 일반 SJF 동점 최초 기록은 세 작업 동시 준비로 유지된다. 이전 관측 두 건은 각각 실제 복원 피연산자 또는 공개 당시 사설 값이 빠져 종료값 2를 유지했다. 수정 SRTF는 저장·복원 정수 31개와 실제 다음 게시 8863→8864를 확인했다. 수정 SJF는 2149번째 시계 호출의 EL1 복귀 중 기한 재점검에서 B/C를 공개하고 A를 유지했다. 실측된 블록 수 2148→2149와 호출 순서에서 복원한 값이 일치한다. 같은 원본 바이트에서 수정된 판독기 종료값은 두 건 모두 0이었다. 변조 대조군 9건은 모두 불일치 또는 증거 공백으로 구분됐다. 신규 게스트 실행은 없었다.
