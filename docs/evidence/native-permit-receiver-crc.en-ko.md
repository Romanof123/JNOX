# IPC-010 controller CRC trial / 제어기 CRC 비교

**Retain the FIFO/RR timing refusal.** Replacing the ordinary Rust client’s bitwise CRC with a read-only lookup table increased the median elapsed interval from loan release to the post-decode clock sample in all four paired cohorts. Each build missed 1 ms on 40 of 512 primary phases. Both implementations agreed with zlib, preserved the expected receiver state and interoperated with the independent privileged bitwise check.

**FIFO와 RR의 1 ms 수용 거부를 유지한다.** 정상 부하의 첫 프레임에서 대여 반환 기록부터 디코드 직후 시계 표본까지 중앙값이 FIFO 209.5→286.0 µs, RR 209.3→280.1 µs로 늘었다. 첫 A 위상에서는 빈 큐를 확인했고 두 구현 모두 기준을 넘겼다. CRC 계산 자체의 단독 시간, 실물 보드의 최악 실행시간 및 운영 승인은 이번 실측으로 결정할 수 없다.

## Cold measurements / 콜드 부팅 실측

Each row contains four baseline and four trial boots in ABBA/BAAB order. All figures below are ticks at 62.5 MHz. A is the bitwise SDK and B the table SDK. Seq 1 is the first accepted frame in each boot. B0 is the first B phase.

| Scheduler / stimulus | Seq 1 release→sample median A → B | Seq 1 at B0 A / B | Later seq 2–8 release→sample median A → B | A0 full upper median A → B | Full-phase misses A / B |
|---|---:|---:|---:|---:|---:|
| FIFO healthy | 13,092 → 17,877.5 | 2 / 3 | 1,224.5 → 1,402 | 98,910.5 → 86,249 | 8 / 8 |
| RR healthy | 13,079.5 → 17,504.5 | 3 / 2 | 1,219 → 1,650.5 | 103,792.5 → 112,186 | 8 / 8 |
| FIFO deliberate overrun | 12,685.5 → 18,329.5 | 3 / 4 | 1,189.5 → 1,313.5 | 92,405 → 86,812.5 | 12 / 12 |
| RR deliberate overrun | 14,183.5 → 17,432 | 1 / 4 | 1,204 → 1,266.5 | 120,195 → 83,333.5 | 12 / 12 |

The release endpoint is the successful controller SVC 36 loan journal, event 6, joined to the received 44 bytes by handle. The next endpoint is the SVC 61 counter operand returned to the closure inside private `Receiver::accept` after `decode`, stored in `admission_controller_rows[i][6]`. For B0 use i=1. This span includes syscall return, private staging, CRC/decoding, entry into INFO, scheduling and host/emulator variation. The protected SVC 60 COMMIT follows the sample. Healthy first-frame release→COMMIT journal medians were 32,122→37,208 FIFO and 32,779→37,377 RR. All six adjacent pairs with seq 1 in the same actual phase (B0) had larger release→sample intervals on B by 4,290–8,555 ticks. Across all 16 adjacent pairs, the delta ranged from −14 to +8,555 ticks. Eight primary boots per cell are a finite comparison.

Every primary boot missed its empty A0 and its first frame, at B0 or A1 depending on actual publication. Every empty B0 met 1 ms. All 16 injected A4 spins exceeded 68,750 ticks and missed the full-phase budget. Primary maxima for A / B were 129,001 / 123,217 ticks for any full phase, 258,248 / 260,819 for a full job, 224,593 / 242,743 between completed checks, and 268,144 / 270,492 for the sender. All finish brackets preceded the next 156,250-tick phase. On-grid IRQ compare→return exceeded the 31,250-tick allowance in each arm (max 32,991 / 31,352). Published deadlines, first release, freshness and recovery conditions were retained.

모든 42회 부팅에서 위상별 사전 대기, 준비, 실제 디스패치, 절대 목표부터 다음 대기 또는 마지막 수신 블록까지의 종료 구간을 확인했다. 1,344개 위상에 열린 구간은 없었다. 같은 B0에서 첫 프레임을 처리한 여섯 쌍은 모두 표 방식의 반환→표본 시간이 더 길었다. 전체 위상 1 ms, 연속 완료 검사 간격 5 ms, 출력 반응 25/5/30 ms는 각각 따로 판정한다. 이번 비교의 범위는 단일 vCPU 가상 출력 벤치다.

## Functional and source evidence / 기능·출처

* The 32 primary boots, four invalid boots, four live-issue boots and two all-client healthy boots produced 71/72/73 exits. Each had 32 protected CHECK/COMPLETE pairs, eight frame transfers, 24 empty polls, expected ranks and zero QUAL/SAFETY losses. FIXED dropped 274–369 later records across all 42. Boundary and payload reductions use the retained QUAL/SAFETY journals and private RAM. Each package manifest, three UART ELF digests, launch record, captured kernel and RAM are bound to files.
* All 336 transferred frames matched sender-private RAM byte-for-byte. Eight accepted commits and ENABLE=1 at 18/37 ms occurred in each primary boot. Each invalid boot had three commits, one private CRC rejection, four NeedsOpen rejections, protected sequence fixed at three after rejection and eventual expiry inhibition. Corrupt inputs had no COMMIT attempt in these guest runs. Live issuance missed exact 10 ms startup spacing in all four controls and remained inhibited.
* Two extra FIFO/RR boots used the newly linked candidate sender, controller and supervisor. Each transmitted generation 1, sequences 1–8, permit 1 on the first seven and 0 on the last. Each accepted all eight and withdrew output. The first 44-byte frame SHA-256 was `4301a4fca01c76e8cbfc1cb632f6a0f972ac3e145706a2d787d48c3253a46f5d` on FIFO and `b8313c33bdfb5c57a25b5ba11c152f13fd2f650c12cf41c63a3a0b7e4a8a696c` on RR. Individual live issue ticks, digests, CRC and bytes are in `results/frames.csv`. These boots are outside the primary comparison.
* Existing SDK tests passed (5 baseline, 6 trial). A separate Rust harness used system zlib for empty, canonical `123456789` (CBF43926), all 256 one-byte and 65,536 two-byte inputs, and 184 deterministic longer buffers. Both implementations matched frozen wire CRC E3566E70. Every one of 320 payload and 32 checksum bit flips produced a private CRC rejection and preserved the accepted state. The harness checked checksum-valid semantic rejection, a valid withdrawal and expiry-first at equality.
* Public baseline `52e4a8b23897801a418da23cac18fefca4cea35b`, tree `aa185fdf2ee73cd1633153ab4ce7ddd6577bf321`. Measured candidate `4cc9254b0957b88bcaa1c5e5f7f54afa15a93764` has the same tree `63d603f00b0d6747574980e06f22c03cbdb9a57c` as public trial `94c54b7da88cb93eaf65c2c2d95c3d0687b500e2`. Only the Rust safety library changed among 712 archived build-input blobs. Both used the same current integrated kernel; the earlier 80-boot study predated subsequent task-policy additions. The current campaign carried forward its ordinary fused-phase argv, release grid, historic sender/supervisor bytes, priorities 6/7/4 and RR quantum 625,000. Serial stock default-clock QEMU 6.2, Cortex-A72, virt/GICv2, 128 MiB, watchdog off, one vCPU, no warmup.
* The stock release SDK built controllers at one source, target and linker-object path. The ELFs were 29,848 / 30,064 bytes, SHA-256 `09ee43be9ec65f02108e0c1ee874165f7bef9324f1f6e319495fde464916968e` / `0a69f36d6537a4032fbde060de14f4e3cf9dcc00e932d25c7f828e80c996a2ba`. Controller .text was 10,208 / 10,740 bytes and .rodata 144 / 1,168. All 256 entries of the linked read-only 1,024-byte table matched zlib. The kernel kept its original source and byte-identical 752-byte privileged COMMIT function within each matched pair. Kernel .text had 166 changed instruction words per comparison (160 ADR, five ADD, one MOV, including embedded ELF addresses/size). Original words, operands and section hashes accompany the archive. Controller-only points always use frozen sender `a126746f393b98c22c69fb92eb26d2883f228a90269633a8a40bdc4d47e33a48` and supervisor `9267d8ae6f318b728522af79f5d1651d9c9fe5b47a6edd08291a087cbb12fa54`.

커널 보호 COMMIT의 명령 바이트가 두 빌드에서 일치했다. 잘못된 프레임을 처리한 네 번의 게스트는 세 번째 순번 이후 승인 상태를 갱신하지 않았고, 실제 발행 시각을 쓴 네 번은 재무장 조건을 충족하지 못했다. 세 앱을 모두 새 SDK로 빌드한 FIFO와 RR에서도 프레임 8개를 수신했고 마지막 철회가 출력에 반영됐다.

## Replay / 재판독

On Linux with Python 3.10+, Rust, zlib development files and GNU AArch64 binutils, extract the archive and run:

```sh
sha256sum --quiet -c SHA256SUMS
bash scripts/replay.sh .
```

The replay reads raw UART and RAM, joins manifest and ELF hashes, walks page tables, recomputes calendars, per-frame CRCs, relative timings, machine-code differences and host test results. For new cold guest measurements with the original cross toolchain, QEMU and both public Git objects available locally, use `bash scripts/reproduce.sh /path/to/JNOX /fresh/absolute/lab`. Timing varies across independent boots. The recorded campaign used the measured trial commit and the offline replay was checked again after extraction.

[Raw captures and offline replay](native-permit-receiver-crc.tar.xz) (SHA-256 `fd84dc5b024154080a1c34c3a864cb677f93aac08ca8fa248eec51b1df43c389`).
