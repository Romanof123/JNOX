# IPC-007 corrected first positions / IPC-007 수정본 최초 실행

Frozen source is commit 73052d1792627c12608df3c65c51d678c40e8eaf, tree 101117ab32df977513fab2624d153fd8d632929d. The only source change from the previous candidate adds the freestanding SDK's signed 32-bit typedef. The independent WSL checkout was clean before compilation. All eight application ELFs and 16 packages built with the public SDK/packager. Each package booted once with QEMU TCG on one Cortex-A72 virt CPU. Its terminal UART, running and paused QMP observations, full physical memory from kernel entry through stack top, kernel ELF, application ELFs and manifest are retained. All guests reached terminal. The two policies used FIFO and RR with quantum 625000; each shell ran at priority 7 and each child at 4.

독립 WSL 체크아웃에서 동결 커밋을 빌드했다. 공개 SDK로 사용자 ELF 8개와 패키지 16개를 만들었다. 각 패키지를 단일 Cortex-A72 virt 게스트에서 한 번씩 실행하고 전체 커널 영역 RAM, 원본 UART, ELF와 매니페스트를 보존했다. 16개 실행 모두 종료 기록에 도달했다. 셸 우선순위는 7, 자식은 각각 4였다.

## Shell / 셸

| Guest input / 게스트 입력 | FIFO and RR observations / 양쪽 정책의 관측 |
|---|---|
| A, 26 bytes, unterminated last line | cat read and wrote 26 bytes and saw EOF. wc read 26 bytes, counted 2 newline bytes and saw EOF. Its PID 2, fd 1 console bytes were 32 0a (hex). Shell, cat, wc exits were 0, 0, 0. |
| B, 823 bytes, consecutive empty lines and final unterminated text | cat read and wrote 823 bytes and saw EOF. wc read 823 bytes, counted 76 newlines and saw EOF. Its console bytes were 37 36 0a. Shell, cat, wc exits were 0, 0, 0. |
| cat missing.txt followed by wc -l | cat exited 1 with zero file/pipe bytes, wc read EOF and printed 30 0a on stdout. Shell reaped child 1 with status 1 and child 2 with status 0, then exited 0. |
| Missing second executable | cat opened the file and read 26 bytes. Its write returned an error after the shell closed the pipe reader. It transferred zero bytes and exited 1. Shell reaped PID 1 and exited 127. Slot 2 remained unscheduled. |
| Malformed command ending at pipe | Shell exited 2. No pipe creation or spawn calls, child dispatches, utility stdout or data transfers. Both registered slots were dismissed. |

Both success cases used the literal shell argument "cat sample.txt | wc -l" and the same three user ELFs. The ROM name was sample.txt in both packages. Replay reads the true file bytes through every process's read-only page tables, reads each process's private argument vector, compares cat/wc private counters and pipe task counters, and reads the compiled console journal directly from retained RAM. Only PID 2 fd 1 supplied those stdout bytes. The live UART also emitted them, and they exactly equal the output of host GNU wc -l on each input. Number of newlines in the empty missing-file stream is zero. Private shell results give live child PIDs, wait return values and statuses. Pipe redirections for cat and wc were shared from shell fd 4 and 3 respectively. Every shell scheduling event was retained. The children had zero service before spawn, first ran at their own ELF entries and never ran in dismissed positions. All pipe objects, linked names, readers, writers, descriptor references and queued bytes retired; all checked SDK ROM file slots were closed.

두 입력의 셸 명령은 같은 문자열이다. 판독기가 각 프로세스의 실제 페이지 테이블, 스택 인수, 카운터, 커널 콘솔 기록을 RAM에서 읽었다. 종료 직전 UART와 PID 2의 표준 출력은 GNU wc 결과와 바이트 단위로 일치했다. 두 입력의 개행 수는 2와 76이다. 정상 입력과 파일 누락에서 셸이 두 자식의 실제 PID를 기다리고 종료값을 회수했다. 실행 파일 누락에서는 첫째 자식만 회수하고 셸이 127로 종료했다. 문법 오류에서는 두 자식 모두 휴면 상태로 해제됐다. 실행 전 자식 CPU 사용량은 0이었다. 해당 셸 실행의 스케줄러 기록은 모두 보존됐다. 파이프 참조·대기 바이트 및 사용자 ROM 파일 기술자가 남지 않았다.

## Pipe streams / 파이프 스트림

The ordinary named FIFO packages used the documented priorities 3,3,3. Each writer committed eight 512-byte records. The reader drained 8192 bytes in 60 positive reads of at most 137 bytes followed by EOF. Replay checks every origin, ordinal, generated payload, in-record CRC and independent writer CRC array, the rejected fifth nonblocking write (−1, EAGAIN 11), and peak occupancy 2048. Exits were 61/62/63. In the anonymous cases, private data contained "rainfall" after the separate transfers "rain" and "fall". The reader saw EOF then EPIPE (−1, errno 32) on its reverse write. Exits were 64/65. The named and anonymous cases closed every object and descriptor.

Two supplementary packages used the identical writer/reader ELFs and wiring. Priorities 7,6,3 place both writers above the reader. Both policies independently recorded four windows in which both writers blocked on a 512-byte write to the full named FIFO. For example, zero-based scheduler event 34 switches from C to Rust with C no longer ready, event 36 switches from Rust to the reader with neither writer ready, and event 48 is a reader read which wakes both. The retained PC at each blocked write is the native write SVC instruction (not the subsequent return PC). C's call site at 0x401694 and Rust's at 0x400200 each load length 512 in w2. The companion disassemblies and frozen call paths distinguish these record writes from the already-completed one-byte handshake. The C and Rust writer block/wake counts were 6/6 and 13/13. The stream still yielded all 16 intact, ordered-per-origin records and no trailing bytes. Their observed global order in all four named captures was C0–7, Rust0–7; the earlier wake of higher-priority C permits it to commit first. Each ordinary named boot dropped 82 late scheduling events and each supplementary one dropped 91. The four contention windows in each supplementary boot were retained before the cutoff. Anonymous boots dropped zero scheduling events.

같은 일반 C·Rust 작성자와 판독자를 FIFO·RR에서 실행했다. 기본 우선순위의 이름 있는 FIFO에서 512바이트 기록 16개, 총 8192바이트와 CRC를 검증했다. 만원에서 다섯째 비차단 쓰기는 EAGAIN을 반환했고 최대 점유량은 2048이었다. 익명 파이프에서는 "rainfall", EOF, 역방향 EPIPE와 종료값을 확인했다. 보충 실행은 우선순위만 7·6·3으로 바꿨다. 두 정책 각각에서 두 작성자가 동시에 쓰기 대기에 들어가고 판독자 읽기로 둘 다 깨어나는 구간이 네 번 남았다. 실제 수신 순서는 C 8개 이후 Rust 8개였다. 기본 이름 있는 FIFO의 후반 스케줄 이벤트 각 82개, 보충 실행 각 91개가 버퍼 한도로 유실됐다. 보고한 경합 구간과 전체 수신 기록·최종 상태는 보존됐다.

Six serialized host model tests also passed for descriptor/object limits, preflight/atomic bounds, access and generations, blocking/open edge cases, partial larger writes and reference accounting. The guest positions above cover the stated workloads. Authorization/recreate/large-write model results are host-side. All scheduler measurements in this packet come from QEMU virt. Physical hardware and performance were outside this run.

추가 호스트 모델 테스트 6개가 통과했다. 권한·세대 교체·큰 쓰기 일부 전송 등의 추가 근거는 호스트 모델에서 얻었다. 실행 대상은 QEMU virt였다.

## Replay / 재현

From the extracted packet, run:

~~~sh
sha256sum -c SHA256SUMS
python3 -B replay.py > /tmp/ipc007-corrected.json
cmp /tmp/ipc007-corrected.json report.json
~~~

The first candidate's SDK failure remains recorded under previous/. Its original archived packet is 379-9271a68-first.tar.gz with SHA-256 23c2818194680b1460c8231c25f8b3fb36ce14e43cf03efddd80dea84ad95a19. run.py contains all original build and launch invocations; every invocation also has its own argv/log. Rebuilding requires the worktree and cross-toolchain recorded in provenance.txt. Offline replay needs the extracted packet and Python 3.8+ with no guest, filesystem mount or network access. sources.tar.gz retains the tracked application, kernel, SDK, board and tool sources. Build-created untracked files and the source commit are listed separately.

압축을 푼 뒤 위의 세 명령으로 원본 해시와 보고서 재생 결과를 비교한다. 이전 커밋에서 발생한 SDK 컴파일 실패 원본과 SHA-256 정보는 previous/에 보존했다.
