# IPC-007 release verification / 네이티브 파이프·셸 출시 검증

**Support acceptance of both IPC-007 proof groups on single-core AArch64 QEMU virt under FIFO and RR.** The release candidate was `1c741462b46309c45965140ae27191f8b538ff3d`, tree `bdc1af8f25f01064171502b347c028d63e00a5d0`. Twelve fresh serialized guest boots used independent builds from a clean WSL checkout. The two earlier public evidence archives also reproduced byte-for-byte from their retained inputs. Hardware and additional scheduling policies are outside this verdict.

**단일 코어 AArch64 QEMU virt의 FIFO·RR에서 IPC-007의 스트림 및 C 셸 그룹 승인을 지지한다.** 독립 체크아웃에서 빌드한 게스트 12개를 순차 실행했다. 이전 최초·수정 패킷도 각각 검증하고 원본 보고서를 동일하게 재생했다. 실제 하드웨어와 다른 정책은 검증 범위 밖이다.

## Native C shell / 네이티브 C 셸

| Fresh position / 새 실행 | Guest result / 실제 결과 |
|---|---|
| Ordinary SDK, FIFO, `cat sample.txt \| wc -l` | 26 file bytes, 2 LF. PID 2 fd 1 emitted `32 0a`. Shell/cat/wc exited 0/0/0. |
| Combined SDK, RR, same command | 672 file bytes, including NUL, CRLF, blank lines, UTF-8 and an unterminated tail. PID 2 fd 1 emitted `36 39 0a` (69 LF). Shell/cat/wc exited 0/0/0. |
| Combined SDK, RR, missing input file | cat exited 1. wc read EOF immediately, printed `30 0a` and exited 0. The shell collected both and exited 0. |
| Ordinary SDK, FIFO, unregistered second executable | cat read 26 bytes. Its write failed after the shell closed the reader. The shell reaped cat with status 1 and exited 127. The unregistered command produced no launch, output or CPU service in the reserved second slot. |
| FD probe, FIFO | The unchanged public shell launched two utilities instrumented at entry. cat fd 1 had `O_WRONLY`, wc fd 0 had `O_RDONLY`, and both descriptor flags were zero. Each opposite-direction call returned −1/EBADF. Both children got −1/EBADF querying the original shell descriptor numbers 3 and 4. The utilities then completed the ordinary pipeline and printed `32 0a`. |

The ordinary SDK build used `--pipes --native-shell`. The combined shell, cat and wc all used `--local-socket --pipes --native-shell`. Source executables receive the command as one shell argument. Private stacks show parsed child vectors `["cat","sample.txt"]` and `["wc","-l"]`. The separate cat/wc ELF identities, private pages, first dispatch at each child's own entry and zero CPU service before spawn were checked. The parent saved PIDs 1 and 2, wait returns 1 and 2, and the two statuses. No scheduling or console events were dropped in these five positions. We compared each successful guest stdout byte to host GNU `wc -l` on the actual ROM input. Both success fixtures stop without a final LF.

기본·결합 SDK 각각에서 별도 ELF인 cat과 wc가 셸의 spawn으로 처음 실행됐다. 게스트 사유 스택의 인수, 읽기 전용 파일 매핑, PID별 전송량·EOF, 실제 콘솔 바이트와 두 waitpid 결과를 비교했다. 결합 SDK는 672바이트의 새 입력에서 호스트와 동일하게 69줄을 출력했다. 기술자 검사에서는 자식의 표준 입출력 방향과 부모 기술자 번호의 부재를 직접 확인했다.

## Streams and lifetime / 스트림·수명

- The named FIFO peaked at 2,048 bytes. The fifth nonblocking 512-byte write failed with EAGAIN and committed no bytes. Both writers were simultaneously blocked on full-record writes in four observed windows. In the first window, scheduler events 34 and 36 make C then Rust non-runnable at the write SVC. Reader event 48 woke both. C and Rust blocked/woke 6/6 and 13/13 times. The reader received 16 complete records, eight per producer, in 60 positive reads of at most 137 bytes plus EOF. Every byte, per-writer ordinal and CRC agreed. The observed order was C0–7 followed by Rust0–7. The final schedule dropped 91 late events. All four reported contention windows and the full received stream are retained.
- The anonymous RR workload duplicated both pipe ends, closed the originals and transferred `rain` then `fall`. The peer read `rainfall`, then EOF. Its reverse write returned −1/EPIPE (32). Both objects retired.
- In the release permission and limit fixture, a foreign process received EACCES on both open directions and EPERM on unlink. An authorized nonblocking writer open without readers returned ENXIO. A 1,900-byte write followed by a 600-byte write committed 1,900 then 148 bytes. Another 500-byte write returned EAGAIN. Recreating the unlinked name produced a fresh empty stream, while the old reader drained and checked all 2,048 bytes to EOF. FD exhaustion yielded EMFILE, and four occupied pipe/FIFO objects yielded ENFILE. The owner exited holding its final two anonymous descriptors. All four objects then had zero names, owners, descriptor references or bytes. Six serialized host tests also passed.
- The first permission fixture reached the descriptor-limit check and exited 100 while setting up the object-limit check. Its higher-priority owner resumed before its peer released the handshake descriptors. One linked FIFO remained after this early exit. The corrected verifier fixture waited for peer EOF before measuring fresh object slots and used a source buffer covering the entire nominal write request. The unchanged kernel passed. Both positions, fixtures and private results are included.

두 작성자가 가득 찬 FIFO의 512바이트 쓰기에서 함께 대기한 구간 네 곳과 판독자의 깨우기를 확인했다. 각 작성자의 기록 8개, 총 8,192바이트를 실제 수신 순서대로 검증했다. 익명 파이프는 양쪽 복제 기술자로 데이터를 인계했고 EOF·EPIPE 뒤 회수됐다. 별도 게스트에서 외부 프로세스의 읽기·쓰기 열기는 EACCES, unlink는 EPERM이었다. 부분 쓰기, 이름 재생성 후 옛 데이터와 새 EOF, 기술자·객체 고갈 및 종료 시 마지막 참조 회수를 확인했다. 첫 보충 픽스처는 동기화용 기술자가 남은 상태에서 일찍 종료했다. 동료의 EOF를 기다린 수정 픽스처의 결과와 최초 실패를 함께 보존했다.

## Integration and provenance / 통합·출처

The ordinary default package's sensor summed [16,32) to 376, yielded four times and exited 17. The AF_UNIX stream exchanged 64 bytes with sum 2080, completed reverse acknowledgment 32 and exited 73/74/0. The regular IPv4 package used an isolated namespace/TAP with a normal Linux socket peer. TCP outbound and inbound transfers, FINs and a guest-observed −1/ECONNRESET matched the ordinary fixture. UDP returned IDs 1,4,3,0 and the peer received DONE. All guest exits were 75/76/0 and both host peer operations completed. The packet retains the wire capture, Linux peer journal and private TCP/UDP results. The first unprivileged network fixture stopped before QEMU because its namespace identity check could not stat /proc/1/ns/net. The unchanged fixture completed from a separate privileged network namespace. The focused local and IPv4 smokes each used FIFO.

기본 센서 합계 376과 종료 17, AF_UNIX 스트림 73/74/0, 격리 TAP의 실제 IPv4 TCP·UDP 75/76/0을 확인했다. 최초 비특권 네트워크 시도는 네임스페이스 식별 권한에서 게스트 기동 전에 중단했다. 같은 패키지·피어 스크립트를 독립 네임스페이스로 실행해 성공했다.

The original 9271a68 packet retained the missing signed 32-bit SDK type, four completed stream boots and zero shell boots. The corrected 73052d1 packet retained sixteen boots under both policies, including the malformed and failed-spawn cases. Both archives passed SHA-256 inventory checks and reproduced their own JSON reports. Their names and SHA-256 hashes are recorded in `prior/archive-hashes.txt`. All fresh package manifests identify the integrated commit and clean tracked source.

최초·수정 패킷에서 각각 셸 SDK 형식 누락과 수정 후 16개 게스트 실행을 독립 재현했다. 재생 기록과 SHA-256은 prior/에 있다. 이번 패킷에는 모든 새 실행의 명령, 원본 UART, QMP RAM, ELF 및 매니페스트가 포함된다.

After extracting the release supplement, verify and replay offline with Python 3.8+.

~~~sh
sha256sum -c SHA256SUMS
python3 -B replay.py > /tmp/jnox-ipc007-release.json
cmp report.json /tmp/jnox-ipc007-release.json
~~~

[Raw guest evidence](native-pipe-ipc-release-supplement.tar.gz) (SHA-256 `2e2737bc1d035794f904edc9afb8a129bf6b76a9303d4202b8beb12fd1475090`).
