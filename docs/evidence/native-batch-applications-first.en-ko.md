# Independent native batch firsts / 독립 네이티브 배치 최초 실행

Candidate 25a41989368d4f76ec7151209e27a448b7b253e2; acceptance source a9fd9a8d1639c796428661c64c52cfd110129b27, #341. Detached Git checkout on WSL ext4, Rust/Cargo 1.95.0, clang/lld 14, QEMU 6.2.0 virt/GICv2/Cortex-A72 unmodified TCG clock, one CPU, 128 MiB, 62,500,000 Hz. The 14 first native boots completed, all private outputs replayed and no decision records were dropped. The SJF equal-queue first missed the planned incumbent-retention observation. It remains the first.

| First / 최초 | Observed / 관측 |
|---|---|
| Arrival SJF and HRRN, plain and traced | At B release A was inside the byte reduction at 0x400808. A retained the CPU and exited, then B ran. |
| Arrival SRTF, plain and traced | PPI30 interrupted A at that same byte load. B ran to exit. A resumed with its saved PC, SP, PSTATE, mapping and frame address, then executed the remaining lease and finished 32,768 blocks. The traced exception stream shows actual EL0 return to B at 0x400000 and later A at 0x400808. |
| All six arrivals | Each A processed 128 MiB, sum 17,112,760,320, exit 17. Each B processed 8 blocks with 2,048 alarms, exit 43. Disassembly proves the sampled PC belongs to the productive inner loop. The capture contains terminal RAM and boundary frame identities. No mid-interruption snapshot of the numeric block count or all integer registers was acquired. |
| Aging | HRRN A exit decision t−E=10,124,297 selected B, SJF 10,086,152 selected C. Both fit the frozen strict cross-product range. C processed 16 blocks, sum 8,355,840, exit 17. Timed A products by slot are recorded in independent-replay.json. |
| Underestimate | A had accrued more than one tick. Both residuals were one at B's actual release, A retained the CPU and both products completed. |
| Queued ties | All three policies returned A, B, C in order. SRTF/HRRN admitted B/C during A. SJF's first timer decision occurred at E+5,442,243, after all three declared releases. It admitted all three once, selected A and preserved B/C order. The distinct SJF arrival firsts demonstrated running A retention. |
| Lifecycle / solo | Yield A→B, B/A sleep, idle/wake A and B, elapsed sleep return and final B yield. Private 53/59, both progress counts two. Wake counters satisfy actual targets and blocked service remains constant. Solo B entered after E+3,125,000 and produced 2,048/43. |
| Compatibility | Six #211 telemetry firsts each exercised two application rejections, three accepts, a fourth live slot with coordinator and a valid request returning full. Each reported 3/0/65535/65542, 3 threshold matches and error 4. All six orders 2,1,0. Three journal replays reconcile exact HRRN fractions, physical frames, SVC returns, four completions and idle. All three historical legacy PT_LOAD comparisons differ. All six ordinary EVD-037 launches and three QMP journal replays passed. Ordinary nonbatch startup probe returned 37 and replayed. |
| Boundary | 13 CLI first rejections, 11 raw build rejections, two raw positives including signed maximum; see controls/results.json for definitive enumeration. C source plus two prebuilt peers with distinct argv and three identical tuples packaged successfully. Full linked scheduling suite 191/191. Host extraction of actual due comparator and checked E+offset, plus duplicate/release/wake/eligible wait/charge controls and linked exact 4 vs 2 and older 5/10 vs fresh 0/1 passed. Overflow and coincident mixed wake/release controls are host evidence. |

## 한국어 판정

14개 최초 실행에서 작업별 사설 메모리의 산출물과 종료 상태를 확인했다. 도착 6건 모두 A가 실제 바이트 합산 루프를 수행하던 시점에 B가 공개됐다. SRTF에서는 B 실행과 A의 저장 PC·SP·매핑·프레임 복원 및 남은 연산 완료를, SJF·HRRN에서는 A 유지를 확인했다. 중단 순간의 누적 블록 수나 전체 정수 레지스터를 별도로 채취하지 않았다. HRRN 노화와 SJF 대조군, 과소 추정, 수면·양보·유휴 및 단독 지연 공개는 정해진 결과를 보였다.

SJF 동점 사례에서는 최초 타이머 처리가 늦어 A/B/C가 처음부터 함께 공개됐다. B와 C가 입력 순서대로 실행됐으나 이 최초 실행만으로는 실행 중 A의 유지까지 관찰하지 못했다. 같은 이미지 재시도 없이 원본을 보존했다. 기존 연결형 텔레메트리 6건에서 실제 코디네이터의 포화 거부와 세 작업의 정확한 산출물을 확인했다. 이전 연결형 적재 이미지 세 개는 달라졌고, 기존 일반 6회 및 저널 3회의 실제 게스트 회귀 검증을 완료했다. 고객 배포 구성과 용량은 #1, 실보드 및 WCET는 #21에 별도 근거가 필요하다.

## Reconstruction / 재구성

first.sh with the candidate's examples/native-batch/README.md generates all five ELF/disassemblies, eleven packages and fourteen captures. Each launch.json records the actual QEMU argv. products/session-sha256 pins the session hashes. Re-run the offline command in first.sh using the retained hash and correct product list; score.py verifies the first-position predicates and exception/ERET order for traced arrival. Host controls are in controls.py and host.py, compatibility launches in compat.sh, original EVD-091 replay and separate PT_LOAD comparison in compat/check-current.py. The historical load comparison failed its equality assertion; the adjusted replay retains that difference and checks the original telemetry assertions unchanged. Native elapsed-sleep/clock transparent calls remain available in external exceptions and aggregate counts. With the signed maximum offset, the raw positive control only establishes successful packaging. No clock adjustment, synthetic interrupt delay, fourth native runtime submission, physical-board run or customer capacity measurement was performed.

