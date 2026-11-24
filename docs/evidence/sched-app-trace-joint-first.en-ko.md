# Joint source first-run verification / 결합 후보 최초 검증

Source 33d8fbfedededb5147d5de93bef044822810c846. 35 distinct release builds and serialized cold boots, Rust 1.95.0 --locked -j1, WSL native ext4, Cortex-A72/GICv2 default TCG. No retries. First-run acquisition, actual SHA-256 inventories and derivations are included.

**Public acceptance fails on all eight wake configurations.** All eight recorded the expected UART, boot exit 0 and seal exit 0. Each unmodified public decoder exited 1 with missing halted.json and withheld its interval. qmp.json records running before stop; the wrapper requested stop before registers and pmemsave but wrote no post-stop query. The independent raw witness calculations below cannot supply the missing observation.


| Position | W | Q | S | E | Q-W | S-Q | E-S | E-W | .text | .data | .bss | .stack |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 01-mlfq-wake-off-0 | 5188368 | - | 7659410 | 7677198 | - | - | 17788 | 2488830 | 16304 | 376 | 73544 | 16384 |
| 02-mlfq-wake-off-1 | 5165208 | - | 7640155 | 7648303 | - | - | 8148 | 2483095 | 17100 | 392 | 364376 | 16384 |
| 03-mlfq-wake-disabled-0 | 5132105 | - | 7595838 | 7601774 | - | - | 5936 | 2469669 | 18356 | 8696 | 73544 | 16384 |
| 04-mlfq-wake-disabled-1 | 5144916 | - | 7616514 | 7622233 | - | - | 5719 | 2477317 | 17668 | 8712 | 364384 | 16384 |
| 05-mlfq-wake-active-0 | 5158751 | 5163694 | 7640303 | 7648088 | 4943 | 2476609 | 7785 | 2489337 | 18372 | 8696 | 73544 | 16384 |
| 06-mlfq-wake-active-1 | 5179672 | 5184873 | 7654497 | 7667987 | 5201 | 2469624 | 13490 | 2488315 | 17668 | 8712 | 364384 | 16384 |
| 07-mlfq-wake-saturate-0 | 5368655 | - | 7686064 | 7698419 | - | - | 12355 | 2329764 | 18372 | 8696 | 73544 | 16384 |
| 08-mlfq-wake-saturate-1 | 5177475 | - | 7644803 | 7654672 | - | - | 9869 | 2477197 | 17668 | 8712 | 364384 | 16384 |

Historical [variant,W,S,E,E-W,gaps] at ec00258: [['plain', 5154439, 7605627, 7614281, 2459842, 8763], ['trace', 5132976, 7612516, 7619398, 2486422, 8845]]. In every new run, six accepted timer IRQs; U=625000, B=14375000 counter ticks. oracle.json includes exact stop-to-start gaps, boundary lateness, pre/post-reset service, and counted packet streams. frames.json separates full journal ELR/frame comparisons from plain ERET transcripts. Actual wake wall differs between independent boots; this measures path elapsed time with scheduler and task work, not causal recorder cost. Stack section is a reservation, dynamic high-water and worst-case IRQ masking duration unmeasured.

Two active raw traces each contain 27 events. Both saturated traces contain the first four events; independent witness replay gives 22 missing kernel events and one missing application event each. Disabled cases have zero commits and drops. OFF has no recorder symbol and historical executable bytes match. All six timer service pairs per active session matched owner/PPI30/instance, with strictly ordered intervals. oracle.json gives per-pair durations and intersection with E-S, without adding them to the task interval. The 8320-byte descriptor exists in each compiled-in build. Producer host tests model pending IRQ/DAIF restoration; no physical nested interrupt, worst-case time or board evidence.

20 historical multilevel cases retain executable .text bytes and expected terminal UART. Timing-dependent IRQ counts differ in some runs. baseline.json lists old/new section equality and exceptions by position. The two basic plain mlq/mlfq positions captured UART and exceptions only; no private RAM/frame dump on those two. The direct ordinary proportional launches captured original UART and exception streams, ELF and disassembly; they did not capture private symbol RAM. The stock assembly bench preserved its own witness.

Both admitted lottery configurations independently passed all 48 SplitMix draws, tickets, winners, private-stack packet receipts, numerical results, 25 overlapping windows and 49 physical IRQ observations including idle. Minima [5,3,11] and [1,7,10], total grants [11,14,23] and [8,17,23]. The tight case refused [24,48), ID 0 count 5 floor 6 before any timer IRQ. Stock bench retained identical executable bytes and 75/6/81 IRQ/SVC/ERET. Five old ordinary .text sections changed; grants/ticks and exception totals matched all five. Expected lottery-miss diagnostics changed source line 446 to 496; code, slot, observed and expected fields (3,0,2,1) matched.


Host controls: five Python decoder tests, three recorder tests including disabled/full and modeled nested IRQ, 139 scheduling controls and one wake-bound test passed. Invalid mode and two foreign mode/profile combinations failed before launch. Numerical and executable compatibility are reported separately from public capture acceptance.

## 한국어 검증 결과

서로 다른 구성 35개를 최초 한 번씩 부팅했다. 깨움 8건 모두 UART와 봉인은 성공했지만 정지 후 상태 파일 누락으로 공개 디코더가 실패했다. 원시 자료에서 네 작업의 실제 패킷 계산과 23U 시간 한도를 독립 확인했다. 활성 기록은 각각 27개였고 포화 사례에서는 각각 커널 22개와 응용 1개가 누락됐다. 타이머 처리 시간을 처리 완료 시간에 중복 가산하지 않았다. 복권 수용 두 건의 배정 48개와 모든 창, 강화 조건의 [24,48) 거절을 확인했다. 기존 다단계 20개와 벤치의 실행 코드가 기준본과 일치했다. 이전 복권·stride 5개의 실제 배정과 서비스 틱이 일치했다. IRQ 차단 중 대기 동작은 호스트 모델로 검사했고 최악 시간과 실기 성능은 측정하지 않았다.

[First combined raw records and independent reconstruction / 최초 결합 원본과 독립 검산](sched-app-trace-joint-first.zip) · SHA-256 64361a6282e3bc36edaa2375a25e48f780d04493463b4440a3a9969039269235.
