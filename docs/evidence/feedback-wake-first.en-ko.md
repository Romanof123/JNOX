# MLFQ wake-to-receipt first verification / MLFQ 깨움·결과 최초 검증

PASS for the four first CPU0 QEMU virt positions at 5301a22176cf102a92485584a1fad4e6d13f1faf. Each position was built once, launched once and recorded before the historical archive was opened. First inputs were the prospective English/Korean design and public build/boot/capture interfaces. Rust 1.95.0 locked release -j1, Linux ext4 workspace, QEMU 6.2.0 TCG Cortex-A72, GICv2, one CPU, 128 MiB. The immutable ELF, UART, CPU register/QMP snapshot, IRQ/SVC log and witness bytes are under each mode. FIRST.sha256 seals all original run files. The per-mode resume-asm.txt files are subsequent disassemblies.

네 구성 모두 최초 빌드·부팅에서 기대 결과를 냈다. 설계 문서와 공개 실행 절차로 먼저 원본을 봉인하고, 이후 과거 결과와 비교했다. 각 첫 실행의 원시 메모리·예외·레지스터·UART·ELF와 빌드 로그를 보존했다.

| Mode / 모드 | W | E | pre-reset owner | post-reset owner | gaps | partial | E−W |
|---|---:|---:|---:|---:|---:|---:|---:|
| plain | 5154439 | 7614281 | 1185362 | 1257063 | 8763 | 8654 | 2459842 |
| trace | 5132976 | 7619398 | 1215977 | 1254718 | 8845 | 6882 | 2486422 |

One U was 625,000 counts. Both runs admitted at 14,375,000. The two refusals rejected 14,374,999 with every runtime word zero and no exception or ERET. In both admitted boots the four initial timer IRQs demoted the four owners. Slot 0 blocked below top rank. At slot 1's wake SVC the FIFO read 2,3,0,1; charging slot 2 crossed the epoch, leaving 3,0,1,2 at reset. Slot 3 received a full top grant ahead of 0. The six IRQs per admitted guest include both subsequent preemptions. Rows, counter addends and sums are available in independent.json. policy.py independently reproduced remaining grants, charge, rank, FIFO order and selected owner on every traced boundary, including unused remaining service at block and wake.

일반 모드의 차단 프레임/복귀 PC는 0x40208e20/0x40202274였고 추적 모드는 0x40208e30/0x40202590이었다. 모든 재배정에서 이전 정지 시의 프레임과 ELR가 다음 프레임 및 QEMU ERET와 일치했다. 두 ELF 모두 블록 복귀 뒤 공급값을 읽어 패킷을 계산하고 영수값을 저장한 뒤 카운터를 읽는다. C로 네 스트림의 32단계 패킷을 처음부터 재계산해 중간 해시, 공급값, 최종 영수값 4개를 각각 대조했다. 일반 모드에서 각 슬롯의 선행 패킷 수는 214807/211315/408937/405754, 추적 모드에서는 216096/210607/424174/425136이었다.

The independent u128 arithmetic oracle passed 1,106,082 admission comparisons including zero, counts near the largest representable bound, overflow, slot cardinalities 0–8, equality and one count below. The project's boundary test also passed. These samples verify the finite virtual profile under its named timing inputs. Hardware latency and deployment workloads require their own observations.

기존 제어 16개도 각각 새 ELF로 한 번씩 부팅했다. eb3d00e4d70738ba37099275575a94bceb03b04b에 등록된 strict-class-owner-window-combined.zip 원본과 UART·정지 PC·실행 코드·RAM 구획 주소 및 크기가 16개 모두 일치했다. 새 admission 심볼은 기존 16개 어디에도 없었다. 전체 ELF 해시는 모두 달랐다. 적재 구획 안에서 차이가 발생한 곳은 .rodata의 Rust 소스 위치 레코드 66바이트였다. 위치별 이전·현재 줄/열은 legacy-analysis.json에 있다.

Historical IRQ counts differed in six of sixteen positions, while all terminal UART/PC outcomes matched. Scheduling replay accepted current MLQ/MLFQ diagnostic traces with 15/42 decisions and the archive's separately labeled historical trace repeats with 17/43. Both strict admissions and refusals passed the receipt checker against current and archived RAM. New blocked/exited traces ended with 2/0 live slots, with timer disabled. Saturation alternated owners 1,2 through the first eight IRQs before serving 0 after the last high exit. The delayed guest crossed three periods on its first IRQ, recorded one reset there, then returned to nominal compare programming. Four resets occurred by termination in this guest, compared with three in the archived follow-up. legacy-limits.json contains the actual timing and counts. These are genuine timing differences. Raw captures include each exact binary and dump. The generic legacy collector waited two seconds and froze symbol RAM, and its footprint and sampling differ from the public wake collector.

기존 진단의 IRQ 수와 행 수가 일부 달랐지만, 스케줄링 규칙·결과·최종 PC를 재생으로 검증했다. 실제 보드 및 제품 최악 지연은 이번 시험 범위 밖이다.

[Independent guest records and replay / 독립 게스트 원본 및 재현](feedback-wake-first.zip) · SHA-256 fe2b54357aa55c0a5caadf4fe07345454ce29ad70736fe9a163d633befeca281.
