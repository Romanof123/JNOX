# Independent native EDF verification / 독립 네이티브 EDF 검증

Public candidate `ffc2e3232cf211f6cb7cda8726f853d26e908f19` includes accepted main `5129f2c88913d06222c9c2af96d03edef0bf34c7`. The prospective criteria were frozen on `d0f10d4ceecfd03cad796f08772926d8dc5d617c`. All kernel packages identify the implementation commit. The first plain, first externally traced and first plain replay used the same three packaged programs and image, one QEMU Cortex-A72, virt/GICv2, TCG, 128 MiB, stock clock. ELF identities, symbols/disassembly, original SDK sources, manifests, commands, UART, exception order, QMP private-memory reads, progress samples and per-event accounting are in the accompanying evidence.

**First plain outcome.** A ran first. The grid-1 release IRQ sampled A in batch_reduce at 0x4001b4. C ran to exit, A resumed with the matching saved PC, SP and frame address, A continued its private computation and exited, then B ran to exit. Slots A/B/C returned 17/29/43. A completed 17,574 blocks with an independently matching u64 sum. B had sixteen bins of 32,768 and C counted 2,048 exceedances. The external first trace and plain replay both interrupted A at 0x40016c, the instruction after the SVC inside clock_gettime. External IRQ/ERET order confirms switching and return to that PC. A single separately predeclared external trace with Linux tmpfs logging also caught 0x40016c. The external trace inside the reduction remains **partial**. Independent full-register equality at the switch was not captured; the directly checked fields are PC, SP and frame identity. Correct subsequent progress and products give an additional functional witness.

**최초 일반 실행.** 격자 1에서 A의 실제 계산 루프를 선점했고, C 종료 뒤 PC·SP·프레임 주소가 일치한 A가 재개되어 계산을 끝냈다. 최초 외부 추적과 최초 일반 재실행, 사전 지정 추가 추적에서는 A의 시계 호출 복귀 주소가 잡혔다. 외부 예외 기록으로 선점과 복귀는 확인했으나 외부 추적으로 계산 루프 내부 선점을 확인한 것은 아니다. 모든 범용 레지스터의 개별 값 비교는 미관측이다.

## Deadlines and timing / 마감과 시간

Frequency 62,500,000 counts/s. Q=6,250,000 counts (100 ms). The independent nonsuspending cost simulation finishes C/A/B at 2/3/11 grids. In the table below each triple uses A/B/C order. Exit elapsed is actual counter minus common S; positive margin is absolute deadline minus actual exit, all in raw counts. Deadlines relative to S are 50,000,000/75,000,000/25,000,000 counts. Results describe these QEMU boots.

| Position | Actual elapsed A/B/C | Deadline margin A/B/C | Release IRQ lateness |
|---|---|---|---:|
| First plain | 10083223 / 10168983 / 6296505 | 39916777 / 64831017 / 18703495 | 10249 |
| First external trace | 10553349 / 10752313 / 6521200 | 39446651 / 64247687 / 18478800 | 118615 |
| First plain replay | 10082837 / 10174211 / 6294831 | 39917163 / 64825789 / 18705169 | 10803 |
| Supplemental external trace | 10084084 / 10186499 / 6299780 | 39915916 / 64813501 / 18700220 | 12497 |

All four passed private products, slot exits, frame PC/SP restoration and deadline comparisons. Each had one involuntary switch and no skipped required grid or dropped journal event. The scores distinguish physical dwell between all changes from grid-driven switches. Only one grid switch per principal run leaves adjacent involuntary dwell unmeasured. Every host poll reads physical private storage; it does not define the instant of the scheduler decision. The host and guest clocks were not synchronized.

수용 모델의 완료 격자는 C/A/B 순서로 2/3/11이었다. 표는 공통 기점 S로부터의 실제 종료 경과와 마감까지 남은 카운트를 구분한다. 네 실행에서 각 슬롯의 실측 마감을 충족했으며 누락 격자·기록 손실은 없었다. 단일 선점만 관측되어 연속 비자발 선점 사이 최소 간격은 판정하지 않았다.

## Independent functional controls / 독립 대조군

| Family | Observation |
|---|---|
| Equal deadlines and delayed first entry | Package order and incumbent held for equal initial deadlines and equal-deadline later release. Single delayed urgent job first dispatched at grid 1. |
| Admission and real miss | (0,1,2) refused with reason 8 and fault 14, zero primary private progress. (0,1,1) modeled exactly at deadline, actually exited at S+10,076,506, late by **3,826,506** ticks (61.224096 ms), with 18,273 valid blocks and exit 17. Both results are retained. |
| Cost exhausted | Single A(0,8,1) consumed 10,001,259 raw owned ticks, exceeding Q, and finished once after later progress with 14,790 valid blocks. |
| Yield, sleep, resume | Equal jobs voluntarily rotated. Both blocked, idle selected first wake while the second remained blocked. Both excluded blocked time from charged service, preserved remaining estimate, private result and request/observed time ordering. Sole survivor yielded to itself. Client checked elapsed sleep. Exits 53/59. |
| Mid-grid wake | Earlier sleeper woke with peer still executing at grid 0. Same peer continued until sampled grid 1. Exits 61/17. Exact coincidence of a physical wake and grid deadline was not forced; source orders calendar processing before sampled selection. Suspension cases are functional evidence. |
| Invalid inputs | Twelve CLI cases and six independent raw Cargo variants rejected. Short text peer refused by package size; separately, header-length text peer packaged and guest refused with fault 4 before any dispatch or exit. Host unit controls cover duplicate job IDs, grid conversion, overflow, final admission cell, model equality and original refusal cases. Deadline −1/equality/+1 classification tested independently. Late-at-arm fault path reviewed but not physically injected. |
| Historical linked EDF / LLF | All eight EVD-048 normal, overrun, overload and traced profiles ran from this candidate. Original offline RAM/UART/exception verifier passed all eight, including grid-5 LLF 2/2 incumbent tie, grid-6 crossover and grid-7 negative slack −2. None missed a deadline. |
| Existing native selectors | Startup exit 37. Cooperative two-program exits 17/43. FIFO/RR/weighted two-program exits 17/43 on extended UART capture. First two-second compatibility captures ended during journal output for those three and are retained. These boot controls do not remeasure historical weighted performance. |
| Host | 147 scheduling cases, 10 masked-supply cases, and historical independent admission-control program passed. Initial host invocation targeted an included source file directly and failed compilation; corrected invocation used its normal scheduling_tests crate. |
| Generalization | Epoch-arithmetic and demand refusal host checks do not establish worst-case physical interrupt latency. Physical board, integrated interference and historical weighted performance retain their previous dispositions. |

비용 1의 A는 실제 CPU 서비스가 Q를 넘은 뒤에도 계속 실행됐다. 별도 마감 1 실험은 모델 수용 뒤 실제로 61.224096 ms 늦게 끝났고 결과와 상태 17이 정상이다. 동률·최초 유휴·양보·서로 다른 절대 수면·중간 격자 복귀·단독 양보와 입력 거부를 확인했다. 기존 연결형 EDF/LLF 8종은 독립 재생 검사를 통과했고, 기존 네이티브 시작·협력·FIFO·RR·가중 선택자도 종료를 확인했다. 물리 보드, 제품 간섭 한계, 정확히 격자와 일치하는 물리 복귀 및 기점 경과 오류의 실기 주입은 이번 증거 범위 밖이다.

## Replay / 재검산

From the extracted directory, run `python3 score.py` for native EDF and `python3 legacy/verify.py` for the eight linked cases. `scores.json` contains each observed compare, timer lateness, skipped count, service sum, owner transition, dwell, private product and deadline margin. Original build and guest launch scripts use the absolute evidence directory and WSL checkout recorded in them. The first prelaunch hash inventory was completed and verified after capture.py had been written, before any guest launched; the earlier build log preserves the initial hash command's missing-file message. See prelaunch.sha256, freeze-verified.log and supplement-prelaunch.sha256. Portable distribution hashes are in SHA256SUMS.
