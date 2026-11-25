# #329 / #330 independent verification · 독립 검증

Candidate ea19c884f07367ce8ab514f2e28e2fffb80e90e7 against main 2901b441adc6388002c13ae8f61ee038fc4cd9fd. The four immutable main firsts passed all 12 measured reporting deadlines. No repeat boots. 원래 순서의 네 최초 부팅에서 12회 모두 40 ms 기한과 산출물 기준을 충족했다. 호환·잘못된 설정은 별도 부팅했다.

|Boot / 모드|Round|Target D ns|Wake sample ns|Publication U ns|U−D ns|Margin ns|CRC blocks during sleep / 대기 중 블록|
|---|---:|---:|---:|---:|---:|---:|---:|
| strict-plain | 1 | 44860816 | 45122080 | 45248576 | 387760 | 39612240 | 297 → 1409 |
| strict-plain | 2 | 85291952 | 85550576 | 85591936 | 299984 | 39700016 | 1849 → 3213 |
| strict-plain | 3 | 125596896 | 125692592 | 125717920 | 121024 | 39878976 | 3381 → 4970 |
| strict-traced | 1 | 43045104 | 43227040 | 43350496 | 305392 | 39694608 | 187 → 1262 |
| strict-traced | 2 | 83400080 | 83512576 | 83566224 | 166144 | 39833856 | 1429 → 2662 |
| strict-traced | 3 | 123576096 | 123688992 | 123728000 | 151904 | 39848096 | 2834 → 3892 |
| feedback-plain | 1 | 43083680 | 43263856 | 43377632 | 293952 | 39706048 | 286 → 1435 |
| feedback-plain | 2 | 83419744 | 83523408 | 83549776 | 130032 | 39869968 | 1649 → 3227 |
| feedback-plain | 3 | 123552256 | 123643232 | 123673200 | 120944 | 39879056 | 3444 → 4793 |
| feedback-traced | 1 | 43166192 | 43350384 | 43472560 | 306368 | 39693632 | 165 → 1242 |
| feedback-traced | 2 | 83522336 | 83654512 | 83685808 | 163472 | 39836528 | 1409 → 2648 |
| feedback-traced | 3 | 123694352 | 123836320 | 123917840 | 223488 | 39776512 | 2817 → 4056 |

|Boot|Reporter exit|CRC exit|CRC blocks|CRC bytes|CRC-32|Retained rows|Lost|
|---|---:|---:|---:|---:|---:|---:|---:|
| strict-plain | 53 | 17 | 28329 | 116035584 | c1390859 | 70 | 0 |
| strict-traced | 53 | 17 | 22098 | 90513408 | 2bdc3d80 | 70 | 0 |
| feedback-plain | 53 | 17 | 28160 | 115343360 | 5be88b1b | 35 | 0 |
| feedback-traced | 53 | 17 | 22016 | 90177536 | 947d79b4 | 35 | 0 |
| compat-plain | 53 | 17 | 27912 | 114327552 | 342090fa | 70 | 0 |
| invalid-plain | 106 | 17 | 28223 | 115601408 | b51a05e4 | 67 | 0 |

Each configured run committed 256/256/256 with cumulative 256/512/768, release markers 1/2/3, three pass verdicts, 12,288 bytes and three blocks. The unconfigured guest retained the original products and exit with unscored markers. Zero milliseconds returned 106 before any round. Host arithmetic controls accepted the largest convertible millisecond value and equality, and rejected a one-nanosecond overrun, zero, malformed and overflow inputs. The normal supported operator reader decoded all six authentic snapshots. 각 유효 설정의 세 표본은 모두 양수이며 동일한 두 ELF를 사용했다. 무인자는 기존 산출물과 종료값을 유지했고 0 ms 입력의 게시 수는 0이었다. 실제 정상 부팅에 지연을 주입하지 않았다.

The ELF stores counts and totals at 0x4007e8–0x400814, calls publication_release at 0x400834 (STLR at 0x4004c8), then issues a fresh monotonic clock call at 0x400840 (SVC at 0x400278). The RAM clock epoch and measured 62,500,000 Hz reconcile each requested target to the exact programmed physical counter tick, with exact integer conversion of 16 ns/count in the guest clock domain. Each wake was accepted at a genuine due IRQ, selected PID 0, and produced later private wake/publication samples. The two traced boots additionally match actual IRQ/ERET PC pairs and the two subsequent reporter clock SVCs per round. The producer commits after its post-wake sample. U bounds completion from above. 표본은 게스트 단조 시계의 카운터 값이다. 실제 보드 또는 호스트 벽시계 최악 실행시간에 대한 결론은 없다.

Private reads bracketed by stable reporter request and pending wake sample show increasing completed CRC blocks in every sleep. Each private stream CRC matches independent zlib over the actual block count. Page-table walks establish distinct private data and stack physical pages. Journal replay matches saved/restored CRC PC and SP, owner/frame and next dispatch. verified.json lists each programmed due tick, IRQ and selected timestamp, counter epoch, saved registers and traced clock ordinals. 비공개 메모리를 읽어 각 수면 중 블록 증가와 CRC 작업의 문맥 복귀를 확인했다.

Both feedback firsts crossed six charged-service boosts. At zero-based row 9 in each, the ready list was [1], reporter asleep, and reset left [1] in order. The immediate dispatch and the next row’s outgoing owner were PID 1. The other five crossings likewise had a singleton eligible queue. The earlier diagnostic establishes the multi-peer order separately. 두 피드백 최초 실행에서 각각 여섯 승격을 재생했다. 이번 실행의 승격 큐 길이는 1이다. 여러 준비 작업의 순서는 아래 원본 근거로 한정한다.

EVD-040 source 91e2e125e68378169853afe31f1cc76b1b186f8b, archive efcc16191cc968647ca8bc517774f2c5804da2e382f838080ca6d1b050b5790c. Hash-checked archive replay verified all 15 MLQ and 42 MLFQ decisions. MLQ rows 1–10 alternated the two high peers, with high exits at 11/12. MLFQ rows 1,4,6,8 preserve cumulative depletion; row 10 preserves [client, interactive peer, CPU] and dispatches client.

EVD-122 source 0ed120045885a75eae2429384bcc6204937f15cc, archive 0b0c5db9e5c4a43d976e09e179b1c3fb3317fa5fe878538d4419dc2833adaaaf. Offline replay verified initial strict 0→1→0, first low at row 67 with both high exits recorded, and feedback PID 0 demotion at row 306 on yield 305. The four native scheduler/ordering/accounting source blobs match this candidate. Equal/cumulative/solo prefixes contain 512 rows and omitted 15,939/15,905/15,886 trailing events. Those tails remain unscored. 원본 통제 실행은 해당 소스·워크로드·남은 행에 한해서만 재사용했다.

EVD-099 corrected finite strict load retained its 24,000,000-count admission limit. EVD-101 retained the four-worker 14,375,000-count wake-to-receipt limit. Both are scoped historical studies. No general lower-class share or hardware deadline follows. 이전 응답시간 연구의 한계와 적용 대상은 각각 유지한다. #21 POL-009의 결정은 독립 Release 검토와 통합 후 PM 권한이다.

The archive contains every original UART, journal, private RAM and enabled exception log, host samples, frozen inputs and commands, linked binaries, disassembly, operator results and independent checks. Per-position hashes are inside session.json. 최초 원본은 모두 WSL ext4에 보존하고 닫힌 결과물만 복사했다.
