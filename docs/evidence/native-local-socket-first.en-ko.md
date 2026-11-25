# #376 local AF_UNIX first verification

Revision `e7d3ded373b58df1a8f520d6a3d3aa3d418eef2a` failed native boot in all six requested combinations. The C application, Rust application, Rust broker and six kernels built. Every unmodified first capture printed all three application SHA-256 digests, then `NATIVE EL0 FAULT 5` before any application exit. The guest produced no socket results. The supplemental socket runtime operations, including peer death and waiter cleanup, could not run on this build.

The native process loader (`kernel/src/native_el0.rs`, `validated_image`) requires three ordered LOAD permissions, RX, R and RW. The C server and Rust client have those three. The broker ELF contains only RX and R. Its two program headers independently identify an unsatisfied loader condition. A separate diagnostic replay of stream/FIFO produced the same guest fault and an empty QEMU exception log. Every original attempt and that diagnostic has its own capture directory and RAM, UART, executable, launch and session files.

| Candidate test | FIFO | RR |
| --- | --- | --- |
| C server, Rust client, Rust broker, stream | FAULT 5 | FAULT 5 |
| C server, Rust client, Rust broker, datagram | FAULT 5 | FAULT 5 |
| C server, Rust client, Rust broker, seqpacket | FAULT 5 | FAULT 5 |
| Socket-disabled C and Rust argument clients | exits 17/43 | exits 17/43 |
| Socket-disabled synchronous C client and Rust server | exits 41/42 | exits 41/42 |

The two ordinary regression packages use the default SDK linkage and omit the socket-service selector. The argument clients produced 376 and 3960 over 16 samples apiece. The synchronous client sent four readings and received the complete summary `0x000404010101082e`. The Rust server's three reply statuses were -116, 0, -116. These products were independently decoded through each slot's own page table in terminal guest RAM. Four existing host engine unit tests also passed. Every QEMU boot used ordinary AArch64 virt, Cortex-A72, one vCPU and TCG, with a 625000-tick quantum for RR.

**Evidence.** `status.txt` and `commands.sh` retain the first attempts in execution order. The capture wrapper returned 1 when a guest produced FAULT 5. `results.json` contains the decoded guest results. To audit on Linux with GNU AArch64 binutils and Python 3, extract the archive, run `sha256sum -c SHA256SUMS`, then `python3 verify.py .`. With QEMU installed, `python3 replay.py . /path/to/new-directory` reboots the ten first-attempt kernels and compares the outcomes. `first.sh`, `remaining.sh` and `sync.sh` preserve the original build/test commands. Full rebuilds need the stated Git revision, Rust bare-metal targets and the repository's toolchain. The first script stops at the first unexpected exit by design. The second continues with uncaptured cases. The issue snapshot came from the PM.

# 한국어

`e7d3ded`에서 스트림·데이터그램·순차 패킷의 FIFO/RR 여섯 첫 부팅 모두 세 ELF 해시를 출력한 뒤 `FAULT 5`로 멈췄다. 일반 로더가 요구하는 세 번째 RW LOAD 세그먼트가 브로커에 없다. 소켓 연산, 상대 종료, 대기 해제를 게스트에서 관측할 수 없었다. 소켓 옵션을 끈 일반 C/Rust 인자 처리와 동기 IPC는 두 정책에서 성공했고, 각 슬롯의 최종 RAM에서 결과를 확인했다. 로컬 AF_UNIX 인수는 미통과이며 실제 IPv4 TCP/UDP 상대 트래픽과 보드 검증은 이 실행 범위 밖이다.

[Download captures and replay / 실행 원본과 재현 도구](native-local-socket-first.tar.gz). SHA-256 `0447796723282912334595938d6372b30c77960917afc068bb0d77c28bd3ccd4`.
