# Receiver private-copy experiment #389 / 수신 전용 복사 검증

**Timing admission remains on hold for FIFO and RR.** Inlining shortened the measured interval from the first accepted RECEIVE journal entry to its RELEASE journal entry on both policies. Every healthy build still had 8 definite misses in 128 controller phases per policy. All 32 healthy boots missed 1 ms twice, first on the empty A0 and again on the phase receiving sequence 1. Every one of those phases had a closed endpoint bracket.

**FIFO와 RR 모두 1 ms 기준을 충족하지 못했다.** 인라인 적용으로 첫 프레임의 수신 기록부터 반납 기록까지 걸린 시간은 대체로 줄었다. 각 빌드는 정책별 정상 부팅 4회, 총 128개 위상에서 8회씩 기준을 넘었다. 프레임 2~8과 전체 마감 초과 횟수에서는 일관된 개선을 확인하지 못했다.

## Healthy runs

One tick is 16 ns. Each row covers four independent cold boots, with four first accepted frames, 28 subsequent accepted frames and 96 empty receives. First-frame figures use the four observations. Warm figures take the median of seven observations per boot, then the median across the four boots. The interval includes the return from receive, guest guards, private copy, release syscall and kernel work through the release journal entry. There is no timer isolating individual loads or the release syscall's subsequent return to EL0.

| Policy | Controller | First receive→release ticks | Warm receive→release ticks | First frame at B0 / 4 | First accepted phase, median upper ticks | 1 ms misses / 128 |
|---|---|---:|---:|---:|---:|---:|
| FIFO | Base | 19,210 | 1,790.5 | 2 | 87,599 | 8 |
| FIFO | CRC | 17,613.5 | 1,748 | 3 | 89,253.5 | 8 |
| FIFO | Inline | 14,844 | 1,718 | 2 | 82,162.5 | 8 |
| FIFO | Both | 13,583.5 | 1,908 | 3 | 92,161.5 | 8 |
| RR | Base | 16,573.5 | 1,819 | 3 | 81,173 | 8 |
| RR | CRC | 18,744 | 1,814.5 | 1 | 87,243 | 8 |
| RR | Inline | 13,223 | 1,777.5 | 2 | 81,251.5 | 8 |
| RR | Both | 12,988.5 | 1,913.5 | 3 | 93,353.5 | 8 |

Within the same order block, base→inline first-frame deltas were −5,198, −4,574, −7,746, −254 ticks on FIFO and −3,587, −3,246, −8,267, +5 on RR. Holding CRC on, adding inline cut the same interval in all four blocks on each policy, with median paired deltas −3,916 and −4,306.5 ticks. Median first-frame times also fell within each observed placement (B0 or A1) for each matched factor and policy, in cells of only one to three samples. Warm paired effects mixed directions. With CRC enabled, the median warm paired delta after adding inline was +134 ticks FIFO and +105.5 RR. The end-to-end first accepted phase could grow despite the shorter copy interval. The four cold observations per policy and arm include translation, host scheduling and guest activity. No board timing, worst-case bound or isolated load-cost estimate follows from this experiment.

전체 정상 부팅에서 첫 프레임이 B0에 놓인 경우는 19회, A1에 놓인 경우는 13회였다. 동일한 순번과 임대 연산끼리 비교했고, B0의 빈 수신과 프레임 도착 시간은 별도로 보존했다. FIFO·RR 모두 두 번째 이후 프레임은 변동성이 남았다. 게스트 시계, CPU, 시간 예산, 스케줄링 우선순위, 발신자와 감시자 바이너리는 모든 정상 빌드에 동일하게 적용했다.

All 1,024 healthy controller endpoints recorded wait-before-release, ready, dispatch and the eventual blocking transition. The largest healthy upper phase endpoint was 130,822 ticks. Across healthy boots the largest upper 5 ms controller job was 266,509 ticks, largest closed sender endpoint 275,023, largest protected CHECK gap 185,376 and largest upper COMPLETE gap 231,843. Maximum observed grid IRQ latency reached 33,598 ticks. All 1,536 fused controller endpoints including controls closed. Qualification and safety event loss was zero. The bounded fixed-schedule debug ring dropped 273–358 rows across fused controls and healthy boots. Timings and byte matches come from the complete qualification/safety rows and terminal RAM.

## Code, scope and linkage / 변경 범위와 바이너리

| Arm | Source used to build | Public commit with identical complete tree | Controller ELF SHA-256 |
|---|---|---|---|
| BASE | 52e4a8b23897801a418da23cac18fefca4cea35b | same | f0e862b1fcbaf3b80e6ca0a5c792588b9a914a68a5e55af127182b0fbf02df90 |
| CRC | 94c54b7da88cb93eaf65c2c2d95c3d0687b500e2 | same | c409869c82076bfcb3544fa13c3cdd64ce08c7e473b5978c047b80056cd902ed |
| INLINE | ca5e50add6bf79f0064eb4b372c3a0274cccd08b | 57300297514fdab612319ddb9305ec0c8732cad5 | 7e0965e729d367492fcfd78cd6c479b7f868c1f84ebe57ed37c1015f514b8061 |
| BOTH | e007c569f21b1180f0d4b63b6696fa3da19cd4de | 91d888d4674eec4a480af301c6c1baff52154d03 | 27a6549f50044f80778322bbfa0922e19e1dedbcdf6847749895c3e397512e78 |

The inline change adds one attribute to the existing unsafe volatile Rust byte accessor. At the two non-inline sites, the controller makes 44 distinct calls with indices 0…43. The 48-byte helper tests offset <4096 before its single indexed LDRB. With inline, both controllers emit exactly 44 one-byte loads from the granted VA at offsets 0…43 and 44 matching private stack byte stores before SVC 36. The out-of-line helper disappears. Main shrinks from 6,052 to 5,680 bytes. The guarded route remains producer 0 to controller 1, length 44, slots 0–2 and a VA equal to 0x01600000 + slot × 4096. Receive SVC 35, return SVC 36, the unsafe lifetime preconditions and the ABI are preserved. Ordinary-reader ELFs independently retain the 4096-byte loop with a checked tail. Resolved function extents, ELF hashes and annotated instruction rows are in results/static.json and static/.

All four indirect source roots used the same kernel, arch, board, tools, controller source and per-case package parameters. Sender SHA-256 a126746f393b98c22c69fb92eb26d2883f228a90269633a8a40bdc4d47e33a48 and supervisor SHA-256 9267d8ae6f318b728522af79f5d1651d9c9fe5b47a6edd08291a087cbb12fa54 were frozen. All four fused linked kernel ELFs and .text hashes differ within each same-policy scenario. The text size is 44,696 bytes for FIFO and 44,700 for RR. The manifests and boot digests bind each result to its actual ELFs. The controller binary is embedded at packaging, so full linked images/layout can covary. This is a four-arm application and package measurement.

소스상 변경은 기존 바이트 판독 함수의 인라인 속성 한 줄이다. 기준 빌드의 함수 호출 44회가 후보 빌드에서 직접 LDRB 44개로 바뀌었다. 반납 전 접근 범위는 두 방식 모두 0~43이며 실제 바이트 수와 사본의 순서도 일치한다. 커널 소스와 빌드 조건은 같고 임베디드 컨트롤러가 달라 링크된 커널의 전체 해시와 텍스트 해시도 달랐다.

## Controls and replay / 대조군과 재판독

Fresh serial stock QEMU Cortex-A72 TCG, virt/GICv2, one CPU, 128 MiB, guest counter 62.5 MHz and normal clock were used. RR quantum was 625,000 ticks. Priorities were controller 7, sender 6, supervisor 4. The shared epoch is obtained on each boot. Sender periods, phase offsets and first-pair issue spacing were checked against that boot's epoch. Four balanced Williams blocks used every arm in every within-policy position; RR reversed each FIFO block. All 56 captures completed on their first attempt within the 40-second per-boot limit. There were 32 healthy boots, eight deliberate A4 overruns, eight corrupt-input boots and eight ordinary loan boots.

All 48 fused boots had 32 CHECK/COMPLETE pairs, 8 received frames and 24 empty receives, with exact 44-byte sender RAM to journal joins. Healthy/overrun accepted sequences 1–8 and ended disabled. The overrun spanned 68,797–69,167 ticks; A4 exceeded its 62,500-tick bound in all eight controls. Corruption tests rejected the bad sequence 4 and its four repeats (codes 2,17,17,17,17). The last accepted sequence stayed at 3 with its original issue value, the fault stayed latched and the endpoint was disabled. The virtual output was high at 18/37 ms before the observed final low transition in each fused boot. Kernel receive/release events aligned with the controller RAM state.

Eight ordinary loan boots exercised lengths 8–29, all 4096 bytes of each of eight pages per boot (payload and zeroed tail), stale-generation rejection −116, full queue −11, producer closed −32, 34 mapping events, producer RW and reader RO/XN, and three cleared pages. All four existing host loan tests passed for every ref. The safety-channel SDK passed five tests on base/inline and six on CRC/both, including the route, slot and length checks.

원본 UART, 종료 시점 RAM, ELF, 각 패키지의 매니페스트, 전체 해시와 원자료로부터 표를 생성하는 스크립트를 포함한다. 전체 56회가 첫 시도에 끝났으며 시간 초과와 제외는 0회다. 일반 대여의 독자 프로세스는 매번 4096바이트를 읽고 길이를 넘는 꼬리가 0인지 검사한다. 8개 오류 주입에서는 다섯 프레임 거부 후 마지막 정상 순번 3과 비활성 상태를 확인했다.

Extract the archive on Linux and run the commands from its root. Offline reduction uses GNU AArch64 binutils, Python 3.10+ and coreutils. Cargo and Rust are needed only to repeat the host suites.

```sh
sha256sum --quiet -c SHA256SUMS
bash scripts/replay.sh .
bash scripts/host.sh .
```

The campaign script records the original boot and build commands and expects the Git object store plus the frozen fixture ELFs. Replay reads only the files in the extracted packet. Per-boot, per-phase, frame, placement, paired timing and provenance tables are under results/.

[Raw captures and offline replay](native-permit-receiver-copy.tar.xz) (SHA-256 `899be455ecc6ac46ef9fd8c87715858c51c62eecda628ce3a60e859bdd87612c`).
