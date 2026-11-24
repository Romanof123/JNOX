# Independent corrected deadline supply assessment / 수정된 CPU 공급 독립 검증

Candidate 608de71f90a76617313aa9046a3741306b10440f, merge base 8609aab7eeb509b7aa032fcf6cafe124770ab751. Four diagnostic and four plain complete cold boots passed. Single Cortex-A72, virt/GICv2, TCG, 128 MiB, 62,500,000 Hz default clock, Q=6,250,000. Rust 1.95.0 Windows-built separate ELFs were run with WSL QEMU. Both original cooperative controls also passed journal, IRQ and idle checks. The independent host ran 138 existing tests and all six new tests. The unequal-deadline MAX/2 vector returns Demand when A alone exceeds four slots by 5. The equal-deadline vector adds both costs and returns Overflow.

| Cold normal | A stop | A at suspend / final blocks | Service restore-to-entry counts | A / B completion margins in counts |
|---|---:|---:|---:|---:|
| EDF trace | 21908987 | 603229 / 897211 | 6233220 | 15499140 / 25991590 |
| LLF trace | 21917275 | 602706 / 899024 | 6223048 | 15500842 / 26003415 |
| EDF plain | 21901590 | 572789 / 853500 | unprinted | unprinted |
| LLF plain | 21909934 | 596988 / 887130 | unprinted | unprinted |

The first complete boots in each mode used EDF refuse, LLF refuse, EDF normal, LLF normal, in order. Both refusals had zero worker rows and zero guest IRQ/SVC entries, with reasons 8 and 9. QEMU exception ELRs, per-worker row charges, completion order and deadline margins match diagnostic UART. At grid 1 A yielded the CPU to service, B arrived, and at 2 A resumed with the saved PC and frame. The saved and resumed A counters agreed before increasing. Both normal traces produced A and B arithmetic values reproduced independently with modular exponentiation. All four normal service stacks reported the expected checksum 89440. A used the printed S+250 ms counter in its initial x0, traced through the frame initializer and linked absolute entry. B retained its relative 30 ms loop. No independent live x0 register probe was made. Plain output omits detailed B blocks, event rows, frames and deadlines. The four-second capture bound covered all terminal markers.

All 10 original LOAD and ELF digests in the archive from a968670a were independently recomputed. Matching the historical WSL toolchain produced byte-identical LOADs for both coop selectors. All eight legacy deadline LOADs changed. EDF .text grew 16 bytes. LLF .text length stayed constant, with changed contents. Byte equality and detailed section sizes are recorded in wsl-compare.json. The ten Windows guest ELFs and ELF/LOAD hashes are retained separately. All eight ordinary legacy guests finished, and the diagnostic phases reproduced EDF C preemption/restoration, LLF tick-5 tie and tick-6 crossover, overload C's negative slack at tick 7, and EDF overrun continuing after exhaustion. Independent timestamped IRQ/SVC, frame restoration, per-worker row accounting and completion deadlines checked. The WSL comparison ELFs were built for hash comparison only.

Trace edges run from CHG-095 through REQ/VER-002,005,023 and IMP-095 to EVD-095, with DES-003/012 and POL-005. Both release case documents link EVD-095 on POL-005, retaining the sampled/interference evidence obligation. This assessment covers finite virtual runs. Physical hardware and product WCET remain unmeasured.

Acquisition history is retained. trace-/plain- prefixes preserve eight actual first launches, truncated because qemu_boot.py --profile deadline expected the older benchmark transcript. full- prefixes preserve 18 pre-guest attempts that failed to find the default NIC firmware. cold- prefixes are separate first complete observations with explicit nodefaults. No failed acquisition was relabeled a successful first launch. The public prospective source document also contains the previous candidate's failure rationale, which was visible with the correction. Historical assessment files were consulted only after the supply captures. The QMP checker required the same run's exceptions.log copied to journal/int.trace. Full bytes and both capture methods are included.

## 한국어 결과

완전한 최초 콜드 캡처는 진단 4건·일반 4건 모두 통과했다. 각 모드에서 EDF 거부, LLF 거부, EDF 정상, LLF 정상 순서를 유지했다. 공급 부족 시 두 정책은 각각 이유 8·9로 작업 시작 전에 정지했다. 정상 실행은 서비스 합계 89440과 A 중단 전·복원 후 양의 진행을 기록했다. 진단에서는 인터럽트·SVC 원문과 작업별 회계, 동일 PC·프레임 복원, A/B 결과값 및 절대 마감 여유를 독립 대조했다. A의 인수는 S+250 ms이며 B는 자신의 시작을 기준으로 30 ms를 사용한다. 초기 x0는 프레임 생성 소스로 확인했다. 일반 출력에서는 개별 완료 시각·프레임과 B 블록 수가 제공되지 않는다.

기존 호스트 시험 138개와 신규 6개가 통과했다. 마감이 5와 7이면 첫 구간에서 A만으로 공급을 초과하여 Demand, 두 마감이 5이면 MAX+2 합산에서 Overflow를 반환했다. 기존 마감 작업 8건이 완료됐으며, 협력식 제어 2건은 QMP 기록·타이머·유휴 PC를 검증했다. 과거와 같은 Linux Rust로 비교한 협력식 2개 LOAD는 동일하고 기존 마감 8개는 달랐다. EDF 기계어는 16바이트 증가했고 LLF는 길이가 같아도 내용이 달랐다. 원본 10개 ELF와 LOAD 해시는 재계산했다. EVD-095는 양 언어 릴리스 표의 POL-005에 연결된다. 대상은 가상 CPU의 유한 실행이며 실물 보드 및 제품 최악 실행 시간은 미측정이다.

초기 8개 게스트 실행은 구형 벤치마크용 수집기가 UART를 중간에 끊었다. 이후 펌웨어 누락으로 게스트 진입 전 종료한 시도도 원본 그대로 보존했다. 표는 독립된 완전한 최초 수집 결과이다. 공개 수정 기준과 함께 표시된 이전 실패 설명을 읽었으며 이전 판정 보고서는 공급 캡처 이후 확인했다.

[Corrected finite deadline supply independent verification](deadline-supply-corrected.zip) · SHA-256 679b8fd6d89849a0a40392c4dd20ba68f4629fb30424488ac80b6d82a75c59df.
