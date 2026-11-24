# #228 main integration design / main 통합 설계

Base 2c3b813e3425391d36f83a0bb788e0025955e3ec. Port the application, Cargo selectors and external capture/replay delta from 7fc8e94..b14b044. Retain main's scheduler, IRQ/bootstrap, journal format, timing and existing fallback. The sole opt-in feature pi4-stream-v1 selects the sample workload on Pi 4 SMP migration-global, migration-partitioned or migration-clustered. Preserve the established all-features virt cooperative selection. Reject other explicit stream combinations.

기준 main의 스케줄러, 인터럽트, 부트스트랩, 저널, 시간 제한과 폴백을 보존한다. Pi 4 SMP의 세 migration 앱에서만 단독 기능으로 스트림을 선택하고 기존 virt 전체 기능 경로도 보존한다.

## Comparison and acceptance / 비교 및 판정

Build three legacy controls on both base and candidate with identical toolchains and flags. Compare images, linked workload code, selector outcomes and legacy freeze/replay behavior. Default M sums 1..65536 in strides of 16, producing 4096 checkpoints, sum 2147516416 and digest 18080812676020141577. Retain each case's artifact hashes, tool versions and any differences.

기능을 끈 세 배치에 대해 같은 도구와 설정으로 기준 및 후보 이미지를 비교한다. 기존 합산 루프와 캡처·재생, 체크포인트 4096개, 합계와 순서 다이제스트를 확인한다.

Stream input is exactly 4194304 bytes. Byte i (zero based) is the low byte of i XOR (i >> 8) XOR (i >> 16). Consume 4096 consecutive 1024-byte blocks once, recording cumulative CRC-32/ISO-HDLC after each. Verify every prefix independently with zlib, final CRC 0x78d22ad1 and ordered digest 8765611335505774534. Release peers after block one. Keep 4 s peer, 20 s idle, 30 s capture and 1 s quiet intervals.

입력 4 MiB 전체를 순서대로 한 번 처리한다. 각 1024바이트 블록 끝에서 독립 zlib 누적값과 비교하고 마지막 CRC 및 전체 순서 다이제스트를 검증한다. 첫 블록에서 피어를 해제하며 시간 제한은 그대로 둔다.

Freeze all three stream ELFs, images, disassemblies, source and stock QEMU (SHA-256 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862) before launch. Capture global 1,2, partitioned 1,2, clustered 1,2 sequentially. Keep each first result including failure. Require linked byte-loop IRQ and ERET at the same PC, X21 state ownership, exact pre/post prefix progression, expected resumed lease (global CPU1, others CPU0), terminal product and all legacy replay checks for each position. Report six distinct model outcomes. Historical 0/6 early finishes and previous selector failures retain their original provenance.

세 배치의 이미지와 실행기를 모두 고정한 뒤 전역·파티션·클러스터 순서대로 각 두 건씩 최초 실행을 보존한다. 각 건의 IRQ·복귀 주소, X21 소유권, 복귀 전후 진행, CPU·리스, 최종값을 개별 판정한다.

[Original workload and corrected selector plan / 원 설계](https://github.com/Romanof123/JNOX/blob/b14b044f1e548f13ddf48bd5bfa5f91001cabcca/docs/boards/pi4-stream-v1.en-ko.md) · [First failed build receipt / 최초 실패 기록](https://github.com/Romanof123/JNOX/blob/b14b044f1e548f13ddf48bd5bfa5f91001cabcca/docs/evidence/pi4-crc-stream-first-builds.en-ko.md)
