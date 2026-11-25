# Native RM/DM corrected calendar verification / 네이티브 RM·DM 달력 수정 검증

Corrected public commit 770c19b147877fb5708e4f044233f73048b1c474, immediate parent fbbae9e0515d2f807288bcf84c397e937a53b111. Accepted baseline 33ee8c35a62ede54987f9af4ae51d3f2fcea3d35, original implementation 4a0c17a7025d663d90b67535ccc27b594277ec08, design 0d2341950ca908eff69c15f2f2574212646b8ce6. Separate Git checkout, serialized builds, host tests and ordinary QEMU TCG Cortex-A72 guests. Every initial position and raw result is preserved.

## Host chronology / 호스트 사건 순서

Two independent generators passed among 161 serial tests. The first enumerates eight explicit jobs with fixed release and deadline times and saves individual completion timestamps. It checks 1,664 snapshots across two policies, deadline lengths 2 and 4, zero to three duplicate polls, increasing backlog and repeated same-tick commitments. The second reconstructs the chronological FIFO for 96 equal-rank arrangements. A separately compiled causal test preserves a real preemption, then checks the old owner after a genuinely late completion. Both independent generators failed on the original implementation. Host-code comparisons supply the exact-tie evidence. All guest results used ordinary counter time.

독립 모형은 각 세대의 공개·마감·실제 완료 시각으로 기대값을 계산했다. 1,664개 상태와 동순위 FIFO 96가지를 확인했다. 원래 커밋은 두 검사에서 실패했으며 수정 커밋은 모두 통과했다. 실제 선점 뒤 새 작업이 종료되고 이전 작업이 한 틱 늦게 대기하는 인과 순서도 별도 검사했다. 정확한 동시각 경계의 증거는 호스트 검사에서 얻었다.

Single slot, period and deadline both 4. First dispatch at 0. Repeated polls at the commitment timestamp give the same answers as direct commitment. Column order is released, completed, overlaps, misses, cancellations.

| Time and commitment | Counts |
|---|---|
| 4, Wait | 2 / 1 / 0 / 0 / 0 |
| 4, Exit | 1 / 1 / 0 / 0 / 0 |
| 5, Wait | 2 / 1 / 1 / 1 / 0 |
| 5, Exit | 2 / 1 / 1 / 1 / 1 |
| 8, Wait | 3 / 1 / 2 / 1 / 0 |
| 8, Exit | 2 / 1 / 1 / 1 / 1 |
| 8, Wait then Wait | 3 / 2 / 1 / 1 / 0 |
| 8, Wait then Wait then Exit (issued gen2) | 3 / 3 / 1 / 1 / 0 |

## Ordinary guests / 일반 게스트 실행

Every one of 22 initial plain/traced positions reached expected exits. The C CRC and Rust histogram used independent address spaces. Both reference variants produced three products and two next-release waits per slot, then exited 17 and 29. RM dispatched C first, DM Rust first. The three reference CRCs were e81722f0, b03637e8 and 82e8647b over 16,384, 16,640 and 16,896 bytes. Each Rust histogram bin was 512, 528 and 544 on successive activations. Fixed-work A1 processed 32 MiB and produced fa914cb6. DM took a physical timer IRQ inside that computation and restored the recorded EL0 PC and SP. The traced exception stream records the IRQ and return. The sleep/backlog firsts each cancelled three releases. Observed misses and overlap counts follow, in slot order. The outside-profile probe exited 59 with ENOTSUP.

22개 최초 실행이 모두 예상 종료에 도달했다. RM은 C를, DM은 Rust를 먼저 실행했다. 각 프로세스의 세 번의 결과와 두 번의 대기, 종료 상태를 확인했다. DM 고정 작업에서는 32 MiB CRC 계산 도중 실제 IRQ가 발생해 다른 프로세스가 실행되었고, 저장한 PC·SP로 복귀해 최종 CRC가 일치했다. 수면으로 밀린 세대 세 개가 두 실행 모두 종료 시 취소되었다.

| Position | Missed generations by slot | Overlaps by slot | Cancellations by slot |
|---|---|---|---|
| dmover-plain | [[1], []] | [1, 0] | [0, 0] |
| dmover-traced | [[1], []] | [1, 0] | [0, 0] |
| dmref-plain | [[], []] | [0, 0] | [0, 0] |
| dmref-traced | [[], []] | [0, 0] | [0, 0] |
| equal-plain | [[], []] | [0, 0] | [0, 0] |
| equal-traced | [[], []] | [0, 0] | [0, 0] |
| rmover-plain | [[1], [1]] | [1, 0] | [0, 0] |
| rmover-traced | [[1], [1]] | [1, 0] | [0, 0] |
| rmref-plain | [[], []] | [0, 0] | [0, 0] |
| rmref-traced | [[], []] | [0, 0] | [0, 0] |
| sleep-plain | [[0, 1, 2]] | [3] | [3] |
| sleep-traced | [[0, 1, 2, 3]] | [3] | [3] |
| solo-plain | [[]] | [0] | [0] |
| solo-traced | [[]] | [0] | [0] |
| three-plain | [[], [], []] | [0, 0, 0] | [0, 0, 0] |
| three-traced | [[], [], []] | [0, 0, 0] | [0, 0, 0] |
| fixeddm-plain | [[1], []] | [1, 0] | [0, 0] |
| fixeddm-traced | [[1, 2], []] | [1, 0] | [0, 0] |
| fixedrm-plain | [[1], [1]] | [1, 1] | [0, 0] |
| fixedrm-traced | [[1, 2], [1, 2]] | [1, 1] | [0, 0] |

Baseline and corrected compatibility guests both returned 37 for startup and 43 for EDF. All four linked control-loss cases reached CONTROL END and their existing analysis verdict was nonqualifying for actual delivery. The raw binaries were copied per case before the next build and compared to the recorded launch digest. Timing varies with instrumentation. Physical board behavior, physical WCET and byte-for-byte stack contents at deschedule were outside these measurements.

기준과 수정 버전에서 일반 시작은 37, EDF는 43으로 종료되었다. 두 버전의 기존 RM·DM 손실 제어는 모두 CONTROL END에 도달했으나 실제 전달 요건을 충족하지 못했다. 정책별 커널 바이너리를 다음 빌드 전에 각각 보존했다. 실물 보드와 최악 실행시간, 중단 순간의 전체 스택 바이트는 측정하지 않았다.
