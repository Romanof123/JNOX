# One additional traced Pi 4 prime pair / Pi 4 소수 계산 추적 1회 독립 검증

**Established for this new run.** On the stock modeled raspi4b, the C and Rust applications alternated four completed productive stores while both vCPUs remained in their own uninterrupted EL0 runs. The same four stores fit between earlier and later completed private stores on both cores. CPU0 and CPU1 had each already returned to an identical saved frame after a genuine local PPI30. This verdict applies to the single additional observation.

**추가 실행에서 확인.** 두 가상 CPU가 각자의 EL0 코드에서 예외 없이 실행되던 구간 안에서 C와 Rust 앱이 실제 완료한 저장 네 건이 교대로 관측됐다. 두 코어 모두 같은 저장 프레임으로 실제 PPI30에서 복귀한 뒤였다. 네 관측 전후에 양쪽 코어 각각의 완료 저장이 하나 이상 더 있었다. 판정 대상은 이번 추가 추적 1회다.

## Identity and disposition / 실행 식별과 원본 보존

- Public reader and collector commit `d7819781ce1995399a67340fa775bfc544c1c9cc`. The unchanged guest package was built from `60475a0ba7c2b1092305043a65d1f6182625a8ed`. Original five-session archive SHA-256 `85c42a7bf664784ec4c643cc69c9a87579685ae01f20ae71f4947d9b533a157c`. The original p2 session is `5875e85633a68623eee42486660012d84464a4362efdc86445214c238b17c720`.
- Before launch, the full new argv, source and decoder digests, terminal cadence, watchdog, bounds and identity were frozen in `prospective.json`, SHA-256 `673e6a7d2c5e214171177e8fd89ef18b77bed056eabbce338fcae6624763b7b7`. Stock seal SHA-256 `61cabffaf0aaf36e2ab601fe802a244b454847a6c16ce29e6045423626d29a53`. The actual launch matched the frozen argv, including the original `-d int`. Both apps retained masks 1/2, priority 2, release cell 0 and argv `/bin/app`. Both timers used the 62.5 MHz counter and 1,250,000-tick grid. The inherited host scheduler was SCHED_OTHER, nice 0, affinity 0–11.
- Exactly one host plugin compiled, one guest launched, zero kernel/app builds. Terminal capture returned 0 with new session SHA-256 `5c10f6cae71d4a4e22b4ac346c1a61761542e82f3069d694f63236d6f64ba70a`. The 25 ms terminal poll, QMP stop, 9,818,112-byte RAM capture and 120 s watchdog used the stock runner. After stop and RAM capture, the runner copied the bounded shared file before killing QEMU. Both 262,464-byte observer files hash to `174c9dd91494a14e49ce530daa05af7e717b0e0c9b9cc0f87a6b6a442ba91283`.
- The original five firsts retain their own unobserved overlap outcomes. Remote traced sender/receiver ordering, global migration, physical Pi hardware and simultaneous host thread execution remain outside this measurement.

기존 p2의 manifest, 이미지, 커널과 두 ELF 및 심볼표는 바이트 단위로 일치한다. 새 disassembly의 차이는 objdump가 첫 줄에 기록한 파일 경로이고 명령 본문은 동일하다. 게스트 재컴파일은 0회, 플러그인 컴파일 1회, 게스트 기동 1회다. 원래 다섯 실행과 원격 인계 판정은 별도로 유지한다.

## Exact interface and capture / 정확한 인터페이스와 수집

The archived Linux x86-64 executable has SHA-256 `651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211`, GNU build ID `f2717a7db511bde6a6b5d68032618c682ca8f56b`, plugins enabled and API-7 header SHA-256 `335d4e472067e914add598b60d1f7cf0dc029cd48f62f66656d6a88c54d8d9f9`. The public source compiled with GCC 11.4.0 and `-Wall -Wextra -Werror` to module SHA-256 `77834ca8b40c2f4b1a7e1fce137f3b69d7afff273c5b113069707f51d0423b20`. Its API symbol is an exported 4-byte integer with value 7. Its installation entry is a function. All 19 imported QEMU functions resolve in the exact binary's 65 public function exports.

The SDK and matched TCG implementation establish vCPU-indexed pre-execution, post-access memory value, width, endianness, RAM PA and architectural-discontinuity mechanisms. The guest store sites and ordinary successors match actual loaded instructions. The collector checks the kernel pre-ERET counter STLR, actual first user instruction after ERET, exception vector landing, and EL1 idle return and landing. Both user ELFs have no WFI/WFE instructions. A bounded MAP_SHARED probe succeeded on the acquisition filesystem. Source availability was qualified before execution. Every live result below comes from this new session.

CPU0 and CPU1 each publish into a 1,024-row buffer shared by their event classes. Journal capacities are 256 each. Sampling occurs every 256 completed stores, with offline ceilings of 307 C and 276 Rust. The C module's shared capacity can drop rows before all per-class maxima in a more eventful run. This first had complete coverage. The kernel and user paths provide identifiable idle exclusion for this workload. The module does not record generic vCPU idle callbacks or direct reads of PSTATE/TTBR at each sample. Ownership and privilege were reconstructed from CPU-indexed returns, disjoint private PAs, RAM roots and CPU-indexed exception PSTATE. The two exit SVC discontinuities report the advanced 0x400020, agreeing with saved ELR on this binary. The SVC itself was at 0x40001c.

동일한 QEMU 실행 파일과 일치 헤더에서 19개 함수 심볼·형식을 확인했다. 모듈은 저장 직전과 실제 다음 명령 도달, 중간 메모리 값·폭·물리 주소를 연결하고 예외 발생 시 미완료 저장을 폐기한다. 코어별 1,024개 행은 여러 이벤트가 공유한다. 사용자 앱에는 WFI/WFE가 없고 실제 EL1 idle 복귀·착지까지 대조했다. 저장마다 TTBR/PSTATE를 읽지는 않는다. 실제 복귀 프레임과 CPU별 예외 PSTATE, 페이지 테이블 및 서로 다른 결과 물리 주소로 EL0 소유권을 검증했다.

## Products, counters and proof / 결과·카운터·증명

| Measured item / 측정 항목 | C on CPU0 | Rust on CPU1 |
|---|---:|---:|
| ELF initial private count / 초기값 | 1 | 0 |
| Terminal private count / 최종값 | 78,498 | 70,435 |
| Signed exit / 종료 코드 | 12 | 77 |
| Store PC → fall-through | 0x40048c → 0x400490 | 0x400084 → 0x400088 |
| Private result PA at VA 0x402000 | 0x2136000 | 0x254b000 |
| Store starts / memory accesses / completions | 78,497 each | 70,435 each |
| Sparse samples / 저장 표본 | 306 | 275 |
| Attempted / committed packets | 386 / 386 | 363 / 363 |
| Discontinuities / actual vectors | 20 / 20 | 22 / 22 |
| User ERETs / first actual EL0 PCs | 10 / 10 | 18 / 18 |
| PPI30 same-frame returns with later stores | 9 | 17 |
| Dropped / aborted / invalid / pending | 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 |

An independent sieve reproduced both prime counts. The second decoder independently authenticated the entire stopped capture and followed each 37-word RAM row, paired 102-word frames and the raw observer packets. All 42 source PCs, vector destinations and source privilege states reconciled to the architectural exception stream. The 28 first user PCs match the prepared frames and return-PC multiset. Register frames, roots, interval generation and private PA identify the interrupted task. The C STR and the Rust STR are both a 32-bit increment in the actual prime-counting loop. For Rust, arrival at the immediately following branch certifies the preceding store.

The independent decoder reproduced the stock generation-1 witness (packet sequences A17, B12, A23, B13) and selected a separate PPI-resumed generation-2 witness below. Clock boundaries in the table already include ±1 ns widening on each relevant end.

| Task / 태스크 | Packet | Count / 저장값 | Store bracket in CLOCK_MONOTONIC ns / 저장 시각 |
|---|---:|---:|---:|
| C / CPU0 | 89 | 20,225 | 254848807696–254848807778 |
| Rust / CPU1 | 39 | 6,400 | 254852646177–254852646249 |
| C / CPU0 | 106 | 24,577 | 254856196045–254856196117 |
| Rust / CPU1 | 46 | 8,192 | 254859965786–254859965858 |

Earlier and later completed stores on both CPUs bracket all four. Packet guards are A79/A121 and B32/B49. The five positive margins, including the two guards, are 3,509,473, 3,838,399, 3,549,796, 3,769,669 and 3,486,977 ns. CPU0's user generation 2 opened at actual PC 0x4004b8, CPU1's at 0x40006c. Each ended at its next CPU-indexed IRQ, with no intervening discontinuity or idle transition. The independent reader matched CPU0 journal rows 1→2 and CPU1 rows 2→3, both with asserted PPI30 and restored identical frames.

두 소수 범위의 정답을 별도 체로 재계산하고 결과 페이지와 종료 코드를 확인했다. 두 코어의 예외 42건과 실제 EL0 첫 PC 28건, 완료 저장 148,932건과 보관 표본 581건을 원본과 대조했다. 두 번째 복귀 세대에서 C 20,225 → Rust 6,400 → C 24,577 → Rust 8,192의 순서를 양쪽의 더 이른·늦은 완료 저장으로 감쌌다. 시계 해상도를 넓힌 뒤 가장 작은 여유는 3,486,977 ns다.

## Guest time interpretation / 게스트 시간 해석

The new startup calibration is [1,179,547, 1,182,995, 1,184,863], giving the CPU1-to-CPU0 correction δ ∈ [−3,448,+1,868] ticks. For generation 2, the measured per-CPU pre-return / exception-entry operands are [3,729,382,4,947,561] and [3,729,985,4,944,094]. For one actual δ the common outer envelope is [max(R0,R1+δ), min(E0,E1+δ)]. The envelope covering any admissible δ is **[3,729,382,4,945,962]**. The intersection of those envelopes over every δ is **[3,731,853,4,940,646]**. Individual store ticks were not sampled. Host nanoseconds establish store ordering; there is no host-to-guest interpolation.

For generation 1, the stock `guest_counter_common_outer_ticks` field reports [2,546,735,3,696,746]. Its formula actually computes the all-offset intersection. The envelope covering every possible δ for that generation is [2,544,647,3,699,843]. The correction governs interpretation of the reported numerical bounds. The host ordering, entry/return continuity and overlap verdict remain independently established on this traced first.

새 보정값은 CPU1 시각을 CPU0으로 옮길 때 −3,448~+1,868틱이다. 2세대의 가능한 모든 오프셋에 대해 사건을 포괄하는 게스트 외곽은 [3,729,382, 4,945,962]틱이다. 각 오프셋별 외곽의 교집합은 [3,731,853, 4,940,646]틱이다. 공개 판독기의 `guest_counter_common_outer_ticks`는 이 교집합을 계산하므로 개별 저장이 그 안에 있었다고 단정할 수 없다. 개별 저장의 게스트 틱을 보간하지 않았다.

## Offline replay / 오프라인 재검증

Extract the accompanying archive on Linux or WSL and run `python3 replay-offline.py` from its root. It checks SHA256SUMS, extracts the original sealed p2 input, runs the public reader and independently reconstructs both witnesses. It authenticates the stock seal then relocates only the original input directory in memory. It never starts QEMU. Raw capture files, exact observer object and header, source hashes, complete prospective argv, pinned helper sources, acquisition scripts and logs are included. The exact 119.6 MB emulator remains identifiable by the public frozen digest and the WSL path recorded in the launch.

Linux/WSL에서 압축을 푼 뒤 루트의 `python3 replay-offline.py`로 전체 체크섬과 두 판독 결과를 검증할 수 있다. 새 부팅은 필요 없다.