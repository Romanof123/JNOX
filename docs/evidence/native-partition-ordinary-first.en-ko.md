# Independent native P/Q first captures · 네이티브 P/Q 최초 독립 검증

Source f7d62aaea34ca4d298c5e052747f4f0e6602f76b. Prospective contract b4d30413277f12a8c156c0b5be2e50778dbb9f4b. WSL ext4, serial independent builds and cold boots, single Cortex-A72 QEMU 6.2, one Cargo job. Each capture directory contains the original RAM, launch record, exception trace, ELFs, UART, probe and hashes. Package manifests identify compiler and linker. All five packages compiled. Historical weighted results supplied no P/Q timing credit.

| Table / clock | Measured windows including extension | Maximum ticks (62.5 MHz) | Original 62,500-tick limit |
|---|---:|---:|---|
| P/default | 10 | 73,392 | FAIL generation 2 |
| P/icount3 | 10 | 1,453 | PASS |
| Q/default | 13 | 55,106 | PASS |
| Q/icount3 | 13 | 1,462 | PASS |

P/default gen 2 selected idle on B's initially unready window following A. d=6,459,487, previous IRQ entry i=6,492,117, selection=6,527,026, last pre-ERET r=6,527,494, first EL0 sample x=6,532,879. The observed components were 32,630 + 35,377 + 5,385 = 73,392 ticks (1.174272 ms). The first interval includes exception-entry uncertainty and the last brackets ERET through the observed user counter read. Neither the hardware/host cause of the first interval nor an exact EL0 transition tick follows from these samples. No replacement run.

The independent crosscheck.py reads linked symbol offsets and raw journal frames, enumerates the original absolute schedule, readiness, counter samples and freshness per return, and matches every ERET/ELR pair in QEMU's exception trace. Repository replay additionally checked physical page walks, stopped private results and frame storage, explicit idle without arithmetic, eligible worker progress, IRQ and bounded exit SVC evidence. Both workers produced positive private counts, exact 3n/5n checksums and exits 41/42 in each run. Three address spaces run the same ordinary static SDK ELF with separate roles. Each Q wrap includes distinct B closures and observations. Per-run JSON contains all timing partitions and exits. Four fresh weighted-share/wake compatibility replays across both clocks had empty timing-miss lists. Independent startup/default passed its own native replay. These results concern normal virtual-target behavior only.

한국어 판정. P 기본 시계의 두 번째 창에서 B가 아직 준비되지 않아 유휴 문맥을 선택했다. 직전 A의 명목 종료부터 IRQ 진입 표본까지 32,630틱, 이후 복귀 직전까지 35,377틱, EL0가 실제로 읽은 시점까지 5,385틱이었다. 총 73,392틱으로 원래 한계 62,500틱을 10,892틱 초과했다. 이 표본만으로 호스트 지연 원인은 확정할 수 없다. 나머지 세 최초 실행은 1 ms 이내였다. 네 실행 모두 최초 준비 상태, 명시적 유휴, 프레임 경계, EL0 복귀 및 인터럽트 PC, 두 작업의 개별 진척과 체크섬 및 정상 종료 41/42를 확인했다. 기존 가중 선택자 두 가지를 두 시계에서 각각 재검증했고 시작 경로도 기본 시계에서 통과했다. 모든 결과는 단일 가상 Cortex-A72에서 얻었다. 최초 실패를 포함한 원본을 보존했다.

[First traces, images and counter replay / 최초 원본과 재생 자료](native-partition-ordinary-first.zip) · SHA-256 46cad0e688b9d279592c83be205ad19505af731a9c103e1df2d80abb45f8e0bf.
