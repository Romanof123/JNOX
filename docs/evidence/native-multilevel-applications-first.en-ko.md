# Independent native MLQ / MLFQ verification · 네이티브 다단계 큐 독립 검증

Public source `0ed120045885a75eae2429384bcc6204937f15cc`. Reference contract `bbf1da20049b7fc9c602e9e2a03974f9cd090970`. Stock QEMU virt/GICv2 Cortex-A72, one TCG vCPU, 128 MiB, default clock, measured CNTFRQ 62,500,000 Hz. WSL ext4 live logs and terminal private RAM bytes. Plans and exact package commands are in freeze.json, build.sh and the manifests.

| First launch / 최초 실행 | Rows / 보존 | Dropped / 누락 | Charged service by slot (ticks) / 슬롯별 누적 | Verified terminal products (app, blocks, bytes, result, exit) / 최종 산출물 |
|---|---:|---:|---|---|
| strict | 70 | 0 | [86136, 40521517] | ['report', 3, 12288, 768, 53]<br>['crc', 25581, 104779776, 3637105911, 17] |
| strict.trace | 70 | 0 | [87068, 40510700] | ['report', 3, 12288, 768, 53]<br>['crc', 19118, 78307328, 37553859, 17] |
| feedback | 35 | 0 | [69616, 40532142] | ['report', 3, 12288, 768, 53]<br>['crc', 27517, 112709632, 4159932031, 17] |
| feedback.trace | 35 | 0 | [68895, 40548290] | ['report', 3, 12288, 768, 53]<br>['crc', 21506, 88088576, 2298869666, 17] |
| three | 75 | 0 | [20424989, 20764232, 27944] | ['crc', 14227, 58273792, 2825066024, 17]<br>['histogram', 21406, 87678976, [5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936, 5479936], 29]<br>['report', 3, 12288, 768, 53] |
| equal | 512 | 15939 | [34572390, 40361757] | ['yield', 16384, 67108864, 4194304, 59]<br>['crc', 26847, 109965312, 3436720467, 17] |
| cumulative | 512 | 15905 | [26503284, 27655924] | ['yield', 16384, 67108864, 4194304, 59]<br>['crc', 18368, 75235328, 3265590264, 17] |
| idle | 8 | 0 | [58260] | ['report', 3, 12288, 768, 53] |
| solo | 512 | 15886 | [25196223] | ['yield', 16384, 67108864, 4194304, 59] |

Both first main pairs ran plain then external exception-traced, strict first. In every main launch the three reporter sleep targets became the earliest compare and the measured entry preceded grant expiry and any boost. The CRC owner handed off at every wake. The external transactions match the saved ELR and next EL0 return PC in both traced runs. In the three sleep windows respectively the background CRC made 890/990/995 clock calls under strict and 1275/1347/1352 under feedback. The clock is sampled after each completed block and once at start. See external-order.json and the ELF disassemblies. Each private report record has three complete 256-count rounds and observed times at or beyond its own target. score.json records release-to-entry and release-to-pre-ERET lower-bound ticks per handoff. The external logs order exceptions and returns without supplying a first-instruction counter timestamp.

고정 클래스와 피드백 각각 최초 일반 실행과 외부 예외 추적 실행을 보존했다. 네 실행 모두 세 번의 수면 기한이 먼저 도래하여 CRC에서 보고 앱으로 즉시 재선택되었다. 독립 예외 기록의 복귀 PC를 저장된 문맥과 대조했고, 각 수면 구간에서 CRC가 여러 블록을 끝낸 뒤 시계를 읽는 것도 확인했다. 복귀 직전의 카운터 표본은 EL0 첫 명령 시각의 하한이다. 보고 앱은 매 회 256건, 누적 256/512/768건을 기록했다.

The strict three-slot winners began 0→1→0. Slot 2 first ran after both higher-class exits. Equal-class voluntary yield and survivor self-reselection appear in the captured rows. Both main feedback launches crossed six anchored boost thresholds. The independent replay checked all charged intervals, FIFO ranks and remainders, pending sleep targets, cumulative service, reset coordinates and saved private ELR/SP. The cumulative control demoted slot 0 on its 305th yield (row 306), before its first boost. Three all-sleep waits contributed zero service or boosts. All long-yield captures preserve a 512-row prefix and a separate terminal record. The discarded suffix is unscored. No naturally coincident earliest compares or multi-level late demotion were seen in scored rows. Host policy tests cover late charge and shared boundary arithmetic. Historical linked and physical-board timings have separate scope.

세 슬롯 고정 우선순위에서 상위 둘은 0→1→0 순으로 교대했고 둘 다 종료된 뒤 하위 앱을 선택했다. 피드백 원본 두 실행은 각각 누적 서비스 기준 승격 여섯 번을 보였다. 자발적 양보 제어에서는 305번째 양보 시 첫 강등이 일어났다. 단독 보고 앱은 세 번 유휴 상태에서 기다렸고 유휴 시간에 서비스 부과나 승격은 없었다. 긴 실행의 앞 512행과 독립 종료 메모리를 보존했다. 누락된 구간, 자연 동시 기한, 여러 레벨을 한 번에 통과한 늦은 인터럽트, 실물 보드 응답시간은 관측 범위 밖이다.

165/165 host tests passed. Eleven invalid CLI cases and ten invalid raw Cargo cases rejected; a valid raw compile passed. Nine legacy selector configurations passed parser checks. See controls.json, controls/, and legacy.json. To recompute from this directory run `python3 score.py` and `python3 trace-check.py`. RAM values are little-endian hex at recorded guest physical addresses. CRC uses host zlib over completed 4,096-byte ramps; all sixteen histogram bins are checked independently.

호스트 시험 165개, CLI 거부 11개, Cargo 직접 거부 10개, 정상 직접 빌드와 기존 선택기 파싱 9개를 확인했다. 종료 메모리의 실제 블록 수를 바탕으로 CRC와 히스토그램 16개 빈을 전부 재계산했다.
