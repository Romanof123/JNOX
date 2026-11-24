# Pi 4 MG2 terminal writer · 독립 검증

Target Rust 7c0295305dd5c0b7b61470fdf5511eb8205c2893, design c5c061624a5dca4211570b9e18066f301cc8932c. All six first boots reached the early-M terminal verdict 0x1d. No interrupted M PC or cross-core M resume was observed. Global requires CPU1 for M, partitioned and singleton clustered require CPU0. None qualified. All six primary captures were cold, consecutive and reopened from D: before the next attempt. No physical-board or speed measurement.

원본 판정으로 0/6이다. 세 배치 모두 첫 두 부팅에서 M이 이주 IRQ 전에 종료했다. 원본 CPU별 예외 기록에는 M의 인터럽트 PC와 복귀 증거가 없다. 전역 배치는 CPU1, 고정·단일 클러스터 배치는 CPU0 복귀가 요구된다. 실물 장치와 성능은 측정하지 않았다.

| First launch / 순번 | Strict PC [start,end), hex | CPU0 M SVC ELR | CPU1 P1 SVC ELR | CPU1 idle ERET | Linked SGIR STR |
|---|---|---|---|---|---|
| global 1 | 82100–82124 | 0x8219c | 0x82298 | 0x80990 | 83954 |
| global 2 | 82100–82124 | 0x8219c | 0x82298 | 0x80990 | 83954 |
| partitioned 1 | 82104–82128 | 0x821a0 | 0x8229c | 0x80994 | 83954 |
| partitioned 2 | 82104–82128 | 0x821a0 | 0x8229c | 0x80994 | 83954 |
| clustered 1 | 821b0–821d4 | 0x8224c | 0x82348 | 0x80994 | 83954 |
| clustered 2 | 821b0–821d4 | 0x8224c | 0x82348 | 0x80994 | 83954 |

CPU1's opening SVC returned to idle via the linked retirement and directed SGIR path. This stock observer records exceptions and ERET, with no direct MMIO transaction timestamp. Retained linked SGIR instruction addresses in the table are static. The SGIR-to-final-add ordering is unresolved. CPU1의 유휴 복귀는 송신 경로의 완료와 일치한다. SGIR 쓰기와 M의 마지막 덧셈 사이의 순서는 미확인이다.

### Journal controls / 저널 검사

All three release ELFs executed their exact linked terminal function in isolated Unicorn instruction controls. Distinct eight-word inputs were checked at each release store. Each image passed all six controls. For 0, 1, 2 free slots the observed tag sequences were [], [13], [13,9] and losses were respectively 2,1,0. CPU mismatch, out-of-range CPU and sealed journal set INVALID with zero publications and zero loss. The linked terminal saves DAIF once, masks IRQ once, reads MPIDR once and restores DAIF once. Each append separately reads the seal and count, retains its own release and has its own exclusive loss-increment loop. CPU identity rejection is shared in the linked branch and performs two INVALID stores. Existing single-event call sites retain their own guard. The clustered SVC calls terminal at 0x8082c, retire at 0x80844 and signal at 0x8089c. See linked-controls.json, linked-sites.txt, the linked map and full disassembly per regime.

별도의 명령어 검사에서 실제 링크된 함수에 서로 다른 8워드 입력을 전달했다. 빈 슬롯 0·1·2개에서 손실 2·1·0과 태그 순서를 확인했다. CPU 불일치, 범위 초과, 봉인 상태에서 INVALID를 기록했다. 보호 구간은 한 번이고 두 행의 release 공개와 용량·손실 처리는 개별이다. 검사용 단일 함수 실행은 여섯 게스트 부팅과 구분된다.

### Reproduction / 재현

Stock QEMU 11.1.3 SHA-256 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862, raspi4b, 4 model CPUs, 2 online, MTTCG, 2 GiB, original per-thread int,tid observer, 30-second ceiling and 1-second terminal quiet. One release build or guest at a time, CARGO_BUILD_JOBS=1, Rust 1.95.0. Build, freeze, full launch argv, ELF/raw/disassembly/map hashes, replay records, reopen manifests and source scripts are included. Previous patched-model and plugin experiments are separate. Stock 모델의 원래 UART·예외 관측을 사용했고 모든 최초 실패를 보존했다. 각 배치별 freeze.json 및 SHA256SUMS에 바이너리 식별값을 기록했다.

[First six captures and linked-journal controls / 최초 여섯 실행 및 링크된 저널 검사](pi4-mg2-terminal-journal-first.zip) · SHA-256 6fbe305df1aef1d4a913afc521c86bc404b98ea337ec589c58fb67b7968aa4c5.
