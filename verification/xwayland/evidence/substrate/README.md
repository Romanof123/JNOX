# Native shared pixmap and socket poll / 익명 공유 파일과 소켓 대기

On source `5cc4884a292fb45844b53e80d4db0766def4ebfe`, the capture team ran one three-process AF_UNIX poll workload and six four-process framebuffer workloads on AArch64 QEMU virt. Two cold boots each exercised the previous fixed-size backing, a sealed anonymous file with ordinary descriptor reception, and a sealed anonymous file with close-on-exec reception. The initial native link failed on the fcntl wrapper when the pipe provider was omitted. The corrected SDK links the real selected provider or its native fallback. [First linker output](initial-link-failure.log)

The poll fixture recorded 20,487,968 ns for its 20 ms timeout, received byte 120 on a subsequent wait, and finished with process statuses 71, 72 and 0. Its bounded UART scheduler log dropped 22 early records. This first capture has terminal RAM. Independent observation of the broker wait and a same-descriptor EOF read are being extended. [Measured poll result](poll.json)

The sealed fixture applies the upstream pixmap allocation pattern, including a shrink seal before growing the file. The capture-time reader observed the installed read-only receiver descriptor with its local close-on-exec bit clear or set as requested, a shared image update and teardown. The standalone init/exec transition for an upstream server is a later acceptance item. [Default FD receipts](default-receipts.json) · [Atomic close-on-exec FD receipts](cloexec-receipts.json) · [Guest probe](../../../../examples/native-xwayland-shm/README.md)

| Before modifying the shared pixels | After modifying the same backing |
| --- | --- |
| ![Before shared update](before.png) | ![After shared update](after.png) |

The captured framebuffer and descriptor phases cover 36 complete 128 MiB memory snapshots. The [lossless RAM archive](full-ram.tar.gz) includes a SHA-256 for each reconstructed snapshot. Extract it to a new directory and run its extract.py with a fresh destination. The files require 4,831,838,208 bytes of logical storage. [Per-boot summary and frame digests](summary.json)

게스트 일곱 번을 실행해 AF_UNIX 대기, 기존 공유 FD, 두 수신 옵션의 익명 파일 전달을 조사했다. 수신자 기술자의 읽기 전용 권한과 close-on-exec 비트, 화면 갱신 및 종료 정리를 기록했다. 이 시험은 JNOX의 서버 전제 기능을 대상으로 한다. Xwayland 실행 파일의 링크와 게스트 내부 실행, 실제 exec 전환은 후속 단계다.
