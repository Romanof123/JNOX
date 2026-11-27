# Xwayland fault signals and shared memory / Xwayland 장애 신호와 공유 메모리

[Native runtime #471](https://github.com/Romanof123/JNOX/issues/471) · [Server #448](https://github.com/Romanof123/JNOX/issues/448) · REQ-010/011/014 · VER-015/024

Xwayland 24.1.6 probes for `sigaction` at link time. JNOX implements that API for HUP, INT, PIPE, TERM and USR1. The upstream SIGBUS recovery code additionally requires synchronous fault delivery with `SA_SIGINFO` and fixed-address anonymous page replacement. The JNOX Meson target defines `JNOX_NO_FAULT_SIGNALS` to describe this boundary. The ordinary DIX control signal and parent readiness paths still install dispositions through `OsSignal`. The server omits optional crash reporters and the SIGBUS recovery object. The event loop uses upstream's existing no-recovery branch. Hardware exceptions continue through the kernel's exception handling.

The software profile disables the X11 MIT-SHM extension because it also requires SysV shared memory. In a build enabling the extension, the target-specific guards limit it to version 1.1. Both request byte orders reject the 1.2 AttachFd and CreateSegment operations with `BadRequest`. The Wayland software renderer can still exchange native memfds through ordinary FD passing.

The shared-FD kernel admits whole-file mappings. A size change checks the object's seals and then every live mapping before modifying the extent. A shrink-sealed file returns `EPERM`. An unsealed file with a live mapping returns `EBUSY`, including when the mapping belongs to another process. The existing guest caller checks that all four mapped bytes and the file size survive the rejection. [Kernel size change](../../../kernel/src/native_shared_fd.rs) · [Mapping lifetime](../../../kernel/src/native_shared_mmu.rs) · [Guest assertion](../../../sdk/native-aarch64/graphics/os/memfd_io_caller.c)

For the serialized server build, inspect generated `include/dix-config.h` for both `HAVE_SIGACTION 1` and `JNOX_NO_FAULT_SIGNALS 1`, with `MITSHM` undefined. The configured build must exclude `os/busfault.c` and retain `os/utils.c`. The [native signal fixture](delivery.en-ko.md) and [mapped-file fixture](../memfd-io.en-ko.md) exercise the two runtime contracts independently. Server startup and the first displayed frame require their own guest evidence.

JNOX는 HUP·INT·PIPE·TERM·USR1의 `sigaction`을 제공한다. Xwayland의 SIGBUS 복구에는 동기 예외의 상세 신호 전달과 같은 가상 주소의 익명 페이지 교체도 필요하다. JNOX 빌드에서는 일반 제어 신호와 부모 준비 통지를 유지하고 선택적 예외 처리기와 SIGBUS 복구 코드를 제외한다. 실제 하드웨어 예외는 기존 커널 경로로 처리한다.

소프트웨어 프로필은 SysV 공유 메모리가 필요한 X11 MIT-SHM 확장을 비활성화한다. 별도 빌드에서 확장을 선택해도 이 대상에서는 버전 1.1까지만 알리고 1.2 FD 요청은 두 바이트 순서에서 모두 거부한다. Wayland의 `wl_shm` 버퍼와 FD 전달은 별도 경로를 사용한다. 커널은 매핑 중인 파일의 축소를 `EBUSY`로, 축소 봉인이 있는 파일의 축소를 `EPERM`으로 거부한다. 게스트 시험은 거부 이후 파일 크기와 매핑 데이터 보존을 확인한다.
