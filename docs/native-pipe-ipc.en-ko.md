# Native pipes and named FIFOs / 네이티브 파이프와 이름 있는 FIFO

CHG-308 · REQ-024 → DES-013 → IPC-007 → VER-024. This plan covers ordinary isolated C and Rust processes on single-core AArch64 QEMU virt with fixed-priority FIFO or RR scheduling. IPC-007 has two proof groups: stream and lifetime behavior in ordinary processes, and an actual native C shell pipeline under REQ-020 / VER-020. Both await evidence. [IPC-007 #379](https://github.com/Romanof123/JNOX/issues/379) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

단일 코어 AArch64 QEMU virt에서 고정 우선순위 FIFO 또는 RR로 실행하는 격리된 일반 C·Rust 프로세스를 대상으로 한다. 일반 프로세스의 바이트 스트림·수명 검증과 REQ-020 / VER-020의 실제 네이티브 C 셸 파이프라인을 각각 입증해야 한다. 두 그룹 모두 검증 전이다.

## Stream and descriptor contract / 스트림과 기술자 계약

| Limit / 상한 | Value / 값 |
|---|---:|
| Pipe objects / 파이프 객체 | 4 |
| Pipe descriptors per process / 프로세스당 파이프 기술자 | 8 |
| Stream capacity per pipe / 파이프당 스트림 용량 | 2,048 bytes |
| Atomic write limit / 원자적 쓰기 상한 (PIPE_BUF) | 512 bytes |

An anonymous pipe returns a readable and a writable descriptor. Each names a process-local reference to a directional open description. A named FIFO requires bounded path storage and explicit rights for the requested read or write direction. Blocking opens rendezvous with a compatible counterpart. Nonblocking read-open can succeed before a writer arrives. Nonblocking write-open with no reader returns ENXIO. Unlink removes the name. Existing descriptions keep the old pipe alive; recreating the name allocates a separate generation. A late open or wake for the old generation cannot attach to the new object.

익명 파이프는 읽기·쓰기 기술자를 하나씩 반환한다. 기술자는 방향이 지정된 열린 파일 설명을 가리키는 프로세스별 참조다. 이름 있는 FIFO는 유한한 경로 저장 공간과 방향별 열기 권한을 사용한다. 차단 열기는 반대쪽의 호환 가능한 열기를 기다린다. 비차단 읽기 열기는 작성자가 오기 전에 성공할 수 있고, 판독자가 없는 비차단 쓰기 열기는 ENXIO를 반환한다. unlink 뒤에도 열린 설명은 이전 파이프를 유지한다. 같은 이름으로 다시 만들면 별도 세대가 되어 이전 대기자가 새 객체에 연결되지 않는다.

dup and dup2 share the open description. dup2 replaces its destination reference and handles an identical valid source and destination without changing counts. Close and exit release references; retirement of the final reader or writer description changes what peers can do. A positive-length read from an empty pipe waits or returns EAGAIN while writers remain. After the last writer closes, buffered bytes drain before the first zero-length result (EOF). A positive-length write with no readers fails with EPIPE. The initial error-return profile covers EPIPE only. Guest SIGPIPE generation, default termination, handlers and ignored dispositions require explicit signal support and separate evidence before a POSIX signal claim. Pin the C errno/return translation, Rust results, native call numbers, descriptor allocation and stdio mapping in the ABI before qualification. [POSIX subset #8](https://github.com/Romanof123/JNOX/issues/8)

dup·dup2는 열린 설명을 공유한다. dup2는 대상 참조를 교체하며 유효한 원본과 대상이 같으면 참조 수를 유지한다. close·프로세스 종료는 참조를 해제한다. 마지막 판독자나 작성자의 열린 설명이 사라지면 대기 중인 상대가 상태를 다시 확인한다. 작성자가 있으면 빈 파이프의 양수 길이 읽기는 대기하거나 EAGAIN을 반환한다. 마지막 작성자가 닫히면 남은 바이트를 모두 읽은 뒤 EOF(읽기 결과 0)가 된다. 판독자가 없는 양수 길이 쓰기는 EPIPE로 실패한다. 최초 오류 반환 범위는 EPIPE까지다. SIGPIPE 생성, 기본 종료, 처리기 및 무시 설정은 신호 지원과 별도 근거가 필요하다. C errno·반환값, Rust 결과, 네이티브 호출 번호, 기술자 할당과 표준 입출력 연결은 검증 전에 ABI로 확정한다.

Writes of up to 512 bytes commit as one contiguous sequence. Insufficient space in nonblocking mode returns EAGAIN with zero bytes committed. Larger writes can return an ordered partial count. Reads may split or combine writes. Valid zero-byte I/O transfers nothing. Check direction, user spans, stale descriptors, object/descriptor exhaustion and authorization before mutation. Keep blocked readers, writers and openers eligible for wakeup when data, space or peer lifetime changes.

512바이트 이하 쓰기는 한 덩어리로 반영한다. 비차단 쓰기에 공간이 모자라면 0바이트를 반영하고 EAGAIN을 반환한다. 더 큰 쓰기는 순서를 유지하면서 일부 바이트 수를 반환할 수 있다. 읽기는 쓰기 경계를 나누거나 합칠 수 있다. 유효한 0바이트 입출력은 데이터를 옮기지 않는다. 방향·사용자 메모리 범위·오래된 기술자·자원 고갈·권한을 변경 전에 확인한다. 데이터·공간·상대 수명이 바뀌면 해당 읽기·쓰기·열기 대기자를 깨운다.

## Independent evidence / 독립 검증

1. **Ordinary processes.** Run two competing writers and a reader under FIFO and RR. Retain actual per-process bytes and returns. Exercise intact atomic records, partial reads and large writes, full/refill transitions, duplicate references at both ends, exact EOF and EPIPE, exit with live references, anonymous cleanup, named open/unlink/recreate, unauthorized use and exhausted limits. Record maximum occupancy, final references, first failures and separate host boundary-race controls.
2. **Native shell pipeline.** Enter `cat sample.txt | wc -l` in the guest's native C shell. The shell must parse the pipeline, create the pipe, launch separate C cat and wc executables, wire stdout and stdin and close unused descriptors. Cat must open and read the guest file; wc must count newlines from actual pipe reads through EOF. Retain command input, file contents, process/FD wiring, output and each child's exit status. The current fixed ELF package starts preconfigured processes. This group also needs guest pathname/file access, shell-driven executable launch with descriptor inheritance or equivalent explicit passing, and child wait/status collection. A preloaded sample or two prelaunched programs cannot establish that path. [Native C environment #12](https://github.com/Romanof123/JNOX/issues/12) · [Interface subset #8](https://github.com/Romanof123/JNOX/issues/8)

1. **일반 프로세스.** FIFO·RR 각각에서 작성자 둘과 판독자 하나를 실행해 실제 바이트와 반환값을 보존한다. 원자 기록, 부분 읽기와 큰 쓰기, 포화·재충전, 양 끝의 복제 참조, 정확한 EOF·EPIPE, 참조를 보유한 프로세스의 종료, 익명 파이프 회수, FIFO 열기·unlink·재생성, 권한 거부 및 자원 고갈을 확인한다. 최대 점유량, 최종 참조 수, 첫 실패와 호스트 경계 경합 결과를 기록한다.
2. **네이티브 셸 파이프라인.** 게스트의 네이티브 C 셸에 `cat sample.txt | wc -l`을 입력한다. 셸이 명령을 해석하고 파이프를 만든 뒤 별도 C 실행 파일 cat·wc를 기동하고 표준 출력·입력을 연결하며 불필요한 기술자를 닫아야 한다. cat은 게스트 파일을 직접 열어 읽고 wc는 EOF까지 파이프에서 읽은 줄바꿈을 센다. 명령 입력, 파일 내용, 프로세스·기술자 연결, 출력과 각 자식의 종료값을 보존한다. 현재 고정 ELF 패키지는 미리 구성한 프로세스를 시작한다. 이 검증에는 게스트 파일 접근, 셸의 실행 파일 기동, 기술자 상속 또는 명시적 전달, 자식 대기·상태 수집이 필요하다. 미리 적재한 데이터나 미리 시작한 앱 두 개만으로는 이 경로를 입증할 수 없다.

## Virtual release / 가상 환경 승인

**IPC-007 · IMP-308 · EVD-308.** On single-core AArch64 QEMU virt under FIFO and RR, isolated C and Rust writers delivered intact 512-byte records through a 2,048-byte stream. A native C shell launched separate cat and wc executables, connected their file descriptors, and reaped both children. On the combined package, wc read 672 bytes and printed exactly `69\n`. The default application, local stream socket and IPv4 peers also completed their integrated smoke runs. [First runs](evidence/native-pipe-ipc-first.en-ko.md) · [Corrected runs](evidence/native-pipe-ipc-corrected.en-ko.md) · [Independent release and raw data](evidence/native-pipe-ipc-release.en-ko.md)

단일 코어 AArch64 QEMU virt의 FIFO·RR에서 두 작성자의 512바이트 레코드와 파이프의 EOF·EPIPE를 확인했다. 네이티브 C 셸은 cat과 wc를 각각 기동하고 파이프로 연결해 두 종료 상태를 회수했다. 통합 패키지에서 wc는 672바이트를 읽고 줄 수 69를 출력했다. 기본 앱과 로컬·IPv4 소켓도 통합 부팅에서 동작했다. [독립 판정과 재현 자료](evidence/native-pipe-ipc-release.en-ko.md)
