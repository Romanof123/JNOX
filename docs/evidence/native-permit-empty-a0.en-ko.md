# #389 — First empty activation / 첫 빈 수신 활성화 A0

**All 32 healthy A0 activations missed 1 ms.** The earliest possible park was 1,191.2–2,092.1 µs after the target. Across the same boots, the latest possible park for any of the 467 later empty A phases was 548.8 µs. The policy split locates two different spans worth measuring more closely. On every healthy FIFO boot the interval from the post-CHECK sample to the SVC35 pre-service sample exceeded the entire CHECK service. On every healthy RR boot the entire CHECK service exceeded that interval. All 16 overrun and invalid-case A0 controls repeated the same ordering and missed 1 ms.

**정상 부팅 32회의 첫 A0가 모두 1 ms를 넘었다.** 목표 시각부터 대기 상태 전환까지 가능한 최소 경과시간은 1,191.2–2,092.1 µs였다. 같은 부팅에서 이후의 빈 A 위상 467건은 대기 전환 시각을 가장 늦게 잡아도 모두 548.8 µs 이내였다. 정상 FIFO 16회에서는 CHECK 종료 표본부터 SVC35 서비스 전 표본까지가 CHECK 서비스 전체보다 길었다. 정상 RR 16회에서는 CHECK 서비스 전체가 더 길었다. 오버런·오류 주입 부팅의 A0 16건도 같은 순서를 보였고 모두 1 ms를 넘었다.

## Healthy first and later empty A phases / 정상 부팅의 첫 A와 이후 빈 A

Values are microseconds from the 62.5 MHz guest counter. Each first-A0 column has 16 boots. Later-empty columns cover 234 FIFO and 233 RR phases. Each later value is the median of the 16 within-boot medians, giving every boot equal weight. Bracketed pairs are medians of independently computed lower and upper bounds. Several rows overlap. Each target-to-park total comes directly from that boot’s samples.

단위는 게스트 카운터 기준 µs다. A0는 정책별 16회, 이후 빈 A는 FIFO 234건과 RR 233건이다. 이후 위상은 부팅별 중앙값을 구한 뒤 16개 중앙값의 중앙값을 표시했다. 대괄호는 관측으로 좁힐 수 있는 하한과 상한의 중앙값이다. 일부 구간은 겹치며, 전체 시간은 각 부팅의 시작·종료 표본 차이로 구했다.

| Sample interval / 관측 구간 | FIFO A0 | RR A0 | FIFO later | RR later |
|---|---:|---:|---:|---:|
| Target → CHECK SVC entry | 474.9 | 486.6 | 114.5 | 115.4 |
| CHECK SVC entry → QUAL-after | 104.2 | 480.9 | 2.7 | 2.7 |
| Within CHECK, site-1 audit → status audit | 32.0 | 378.1 | 0.6 | 0.5 |
| CHECK-after → SVC35 pre-service¹ | 619.3 | 124.4 | 23.1 | 25.9 |
| SVC35 pre-service → empty receive audit¹ | 133.6 | 148.2 | 1.0 | 1.0 |
| Target → park | [1,750.3, 1,751.2] | [1,756.1, 1,757.2] | [234.3, 235.0] | [237.5, 241.4] |

¹ The FIXED prefix retained these two split intervals in 58 of 234 later FIFO phases and 57 of 233 later RR phases, covering all 16 boots of each policy. Both are available for every A0. Across all 32 healthy boots the observed A0 SVC35 pre-service → empty audit was 129.0–234.5 µs. Every retained later observation was 0.7–11.2 µs. The other 352 later splits are interval-censored. Their complete CHECK-after → receive audit remains measured. The reducer gives bounds for the missing internal timestamp and excludes it from the observed-only medians.

¹ 이후 빈 A 중 FIXED 세부 표본은 FIFO 58건, RR 57건에서 남았다. 두 정책 모두 16개 부팅이 포함됐다. A0에서는 모두 관측됐다. SVC35 서비스 전 표본부터 빈 수신 기록까지는 A0에서 129.0–234.5 µs, 이후 세부 표본이 남은 위상에서 0.7–11.2 µs였다. 나머지 352건에서는 내부 경계 시각의 범위만 알 수 있다. CHECK 종료부터 수신 기록까지 전체 구간은 이 경우에도 실측했다.

## Where the first target time goes / 첫 목표 시각 이후의 진행

| First A0 interval, median µs / A0 구간 중앙값 | FIFO | RR |
|---|---:|---:|
| Target → physical timer IRQ entry sample | 173.9 | 166.9 |
| IRQ entry → ready state transition | [107.1, 119.8] | [102.0, 114.9] |
| Target → ready state transition | [279.7, 293.6] | [272.7, 285.2] |
| Ready transition → first resumed EL0 execution | [139.6, 161.8] | [143.6, 169.5] |
| First resumed EL0 execution → CHECK entry | [51.1, 60.4] | [51.2, 63.8] |
| Empty receive audit → COMPLETE SVC entry | 99.4 | 115.0 |
| COMPLETE SVC entry → QUAL-after | 52.8 | 40.2 |
| COMPLETE-after → stage-3 SVC entry | 99.6 | 104.5 |
| Stage-3 entry → next phase’s arm SVC entry | 43.2 | 44.4 |
| COMPLETE-after → actual park transition | [145.6, 146.6] | [150.3, 151.3] |

For all 768 A positions, the RAM epoch and nominal stage target agreed with the last requested comparator and hardware CVAL. The controller blocked before its target. A single matching enabled and pending PPI 30 was sampled from idle. The same scheduler row made PID 1 runnable and selected it. Its saved return PC matches the controller’s armed-mark return PC. The target is a programmed counter operand. The first IRQ sample already includes delivery and exception-entry latency. Ready lies within the old/new runnable-mask samples. The actual first user instruction lies between the return-to-user assembly sample and the next synchronous stage-2 SVC entry. The IRQ’s own return from idle can precede the actual user return. On healthy A0 the ready bracket was 9.8–20.9 µs wide, the first-execution bracket 7.9–19.0 µs and the final park bracket 0.8–2.4 µs. A new blocking scheduler transition after arming B0 establishes the endpoint.

전체 A 위상 768건에서 RAM의 기준 시각, 마커의 목표, 비교기 요청값과 하드웨어 CVAL이 일치했다. 컨트롤러는 목표 전에 대기했고, 유휴 상태에서 일치하는 타이머 IRQ를 받았다. 같은 스케줄러 행에서 PID 1이 실행 가능 상태가 되고 선택됐다. 준비 시각은 스케줄러의 상태 변경 전후, 첫 EL0 실행은 복귀 직전 표본과 단계 2 시스템 호출 사이, 최종 대기는 다음 B 위상을 예약한 뒤 실제로 차단된 전환 전후로 한정했다. 타이머 목표부터 첫 IRQ 표본까지에는 전달과 예외 진입 지연도 들어간다.

## Cold-path distinctions / 첫 실행에서 갈리는 구간

- **FIFO.** Across 16 healthy A0s, CHECK-after → SVC35 pre-service ranged from 218.9 to 1,004.6 µs. The entire CHECK service ranged from 98.7 to 151.4 µs. Qualifying CHECK ends before scheduler work and architectural return. The next endpoint is inside the SVC35 handler after exception entry and validation. Between them the controller begins its activation, opens its private receiver session on A0 and calls receive. The available clocks leave dispatch/return, EL0 execution and new SVC entry combined.
- **RR.** Across 16 healthy A0s, the entire CHECK service ranged from 231.9 to 1,040.7 µs. The site-1 event-12 audit → event-5 CHECK status audit alone ranged from 154.5 to 948.2 µs, versus 27.8–37.7 µs on FIFO. This bracket includes the remainder of the first journal append, protected CHECK activation bookkeeping and the path into the status journal’s counter read. It excludes the earlier site-1 check operand→audit interval and the later state readback, both recorded separately in the CSV.
- **Empty receive.** Every selected A0 issued exactly one SVC35, returned −11 with no loan, and recorded no release or COMMIT. The compiler’s frame-copy routine is never reached on this path. Later comparator phases also have one empty SVC35 and a successful fused CHECK/COMPLETE pair. Their private receiver session was already open. The RAM status, private/accepted sequence, open indicator and fault flag remain available per row. Overrun A4 has an injected delay and its own cohort in the summary.

FIFO에서는 CHECK 종료 뒤부터 빈 수신 서비스 직전까지 218.9–1,004.6 µs가 걸렸다. 이 구간에는 커널 복귀 처리, EL0의 최초 활성화·수신기 세션 열기, 새 시스템 호출 진입이 섞여 있다. RR에서는 보호된 첫 검사 기록부터 CHECK 상태 기록까지가 154.5–948.2 µs였다. 같은 구간의 FIFO 범위는 27.8–37.7 µs였다. 이 기록은 저널 작성의 일부와 활성화 상태 갱신을 함께 감싼다. A0는 48건 모두 수신 한 번이 −11로 끝나 복사·반납·COMMIT 경로에 들어가지 않았다. 이후 위상과 달리 A0에서는 수신기의 전용 세션을 처음 연다.

## Base and inline / 기본 및 인라인 빌드

Each healthy cell has four separate boots. Values are medians in µs. The final column uses the lower park bound. The per-boot table retains both bounds, and the paired table subtracts bounds conservatively in the same policy/order block.

| Policy | Build | CHECK site-1 audit → status | CHECK-after → SVC35 | SVC35 → receive audit | Target → park lower |
|---|---|---:|---:|---:|---:|
| FIFO | Base | 31.2 | 642.6 | 144.4 | 1,750.3 |
| FIFO | Inline | 31.3 | 475.7 | 133.4 | 1,600.1 |
| FIFO | CRC | 29.0 | 428.0 | 132.0 | 1,545.9 |
| FIFO | Both | 34.5 | 516.9 | 141.5 | 1,661.2 |
| RR | Base | 163.1 | 122.4 | 133.9 | 1,444.8 |
| RR | Inline | 503.2 | 131.7 | 148.8 | 1,766.2 |
| RR | CRC | 375.8 | 130.1 | 191.6 | 1,822.5 |
| RR | Both | 599.0 | 135.5 | 148.2 | 1,765.1 |

The observed base→inline total went up in two of four FIFO blocks and three of four RR blocks. With CRC already enabled, the total went up in two FIFO blocks and fell in all four RR blocks. The empty A0 miss persisted in all cells. The traces identify the mixed post-CHECK gap on FIFO and the protected journal-to-journal gap on RR as distinct candidates for finer measurements. They supply elapsed guest-counter time, which includes QEMU translation, host scheduling and instrumentation wherever those occur. The evidence does not isolate their individual costs, establish a change-specific cause, or bound board WCET.

기본→인라인의 전체 시간은 같은 순번끼리 비교할 때 FIFO 4쌍 중 2쌍, RR 4쌍 중 3쌍에서 증가했다. CRC를 켠 상태에서 인라인을 추가하면 FIFO의 2쌍에서 증가했고 RR의 4쌍에서 모두 감소했다. 모든 빌드에서 빈 A0의 기한 초과는 남았다. 다음 측정에서 좁힐 위치는 FIFO의 CHECK 종료 뒤 복합 구간과 RR의 보호 저널 기록 사이 구간이다. QEMU 번역·호스트 스케줄링·계측이 섞인 게스트 경과시간이며 각 비용이나 변경의 인과 효과, 보드 최악 실행시간을 확정할 수 없다.

## Receipts and offline replay / 근거 및 오프라인 재현

The frozen campaign contains 56 boots. The reducer reads all 48 fused boots across four revisions and both policies, and inventories every A phase. Of 768 positions, 748 were empty and 20 carried frames. Ordinary loan fixtures have no fused A0. Replays verify the source archive or pinned source manifest, selected member hashes, individual capture receipts, QUAL/SAFETY loss, compare control/target, qualified scheduler/return and service samples, protected loan records and controller RAM. The per-phase source CSV independently agrees on marker, ready/dispatch, receive and park endpoints. A second replay from authenticated RAM and ELF page tables reproduced all 1,536 controller rows, phase brackets and role headers. Fourteen in-memory mutations were refused. [Source campaign report](https://github.com/Romanof123/JNOX/blob/a7aa0a659f239950fa29b3326813cf4b35d846f9/docs/evidence/native-permit-receiver-copy.en-ko.md) · [Frozen capture archive](https://github.com/Romanof123/JNOX/blob/a7aa0a659f239950fa29b3326813cf4b35d846f9/docs/evidence/native-permit-receiver-copy.tar.xz)

원자료 SHA-256은 `899be455ecc6ac46ef9fd8c87715858c51c62eecda628ce3a60e859bdd87612c`다. 독립 해제 폴더와 원본 압축 묶음에서 같은 결과를 재현했다. 원시 RAM·ELF의 재판독에서도 컨트롤러 행 1,536개가 모두 일치했고, 손상·순서·경계·상태 오류를 넣은 대조 14건은 예상대로 거부됐다.

From the extracted analysis packet, use Python 3.10+ and an empty output directory. The first command takes the original source archive or its extracted root. It performs no builds or boots.

~~~sh
python3 -B reduce_a0.py /path/to/native-permit-receiver-copy.tar.xz replay
diff -r results replay
~~~

Expected totals are 48 fused boots, 768 A-phase positions, 748 empty receives and 14 refusal controls. CSVs provide each raw clock and interval, boot-weighted medians/ranges with denominators, and paired healthy contrasts. To repeat the full raw-RAM verification, extract the source archive on Linux with GNU tar, install GNU AArch64 binutils and run `python3 -B replay_ram.py /path/to/extracted/source`.

[Raw captures and offline replay](native-permit-empty-a0.tar.gz) (SHA-256 `6a86c800a8581434faa1c02b05c84a31f602de7121d427326e915660b601e842`).
