# Picolibc sleep on AArch64 / AArch64 Picolibc 대기

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. The upstream Xwayland 24.1.6 `os/utils.c` calls `sleep(2)` twice in `LockServer` when retrying lock-file operations. These calls require an elapsed duration independent of calendar time. [Server link #471](https://github.com/Romanof123/JNOX/issues/471) · [Native absolute wait](../../../docs/native-absolute-sleep.en-ko.md)

`unsigned int sleep(unsigned int seconds)` in Picolibc's `<unistd.h>` uses the native monotonic clock and absolute timer services (6 and 7). A zero request returns immediately. A positive request adds whole seconds to the sampled deadline with checked arithmetic and preserves its nanosecond fraction. The kernel rounds the target up to the first representable counter tick. The caller remains blocked until its task can resume. A successful wait returns zero and preserves `errno`. [SDK implementation](../../../sdk/native-aarch64/graphics/os/sleep.c) · [Kernel service](../../../kernel/src/native_el0.rs)

An initial clock failure or deadline addition overflow returns the requested seconds and sets Picolibc `errno`. An error from the absolute wait is reported through `errno`; the adapter samples the remaining monotonic interval, rounds a fractional second up and caps the result at the original request. If that second clock read fails, the full request is returned with the original wait error. The current native process profile wakes a sleeping task on its deadline and has no POSIX signal delivery to a sleeping frame. A real signal-interrupted sleep needs separate guest verification once signal delivery is available.

The AArch64 [guest caller](guest.c) includes the public Picolibc headers. Compile it using the graphics runtime cross file and the installed `include/jnox-wayland` overlay, link `libjnox-wayland-os.a` with the Picolibc graphics runtime, then package as the first FIFO application. Expected guest exit 77 requires a zero sleep, an unchanged `errno`, and a completed two-second sleep bracketed by `CLOCK_MONOTONIC` observations. There is no upper bound on dispatch latency in this check.

Run the separate LP64 arithmetic test using the production implementation and a controlled clock backend. Exit 0 checks the full unsigned input range, deadline overflow, clock failure, and interrupted waits before, at and after the target. The signal case in this test supplies a return code to the adapter.

```sh
cc -std=c11 -Wall -Wextra -Werror -I sdk/native-aarch64/include \
  sdk/native-aarch64/graphics/os/sleep.c verification/xwayland/sleep/edges.c \
  -o "$OUT/sleep-edges"
"$OUT/sleep-edges"
```

Xwayland는 잠금 파일 처리를 다시 시도할 때 `sleep(2)`를 호출한다. JNOX는 네이티브 단조 시계에서 읽은 시각에 요청한 초를 더하고 커널 타이머로 절대 기한까지 대기한다. 0초는 즉시 성공한다. 기한 덧셈이 넘치면 요청량과 `EOVERFLOW`를 반환한다. 대기 오류 시 남은 시간을 초 단위로 올림해 반환하고 원래 오류를 보존한다. 게스트 호출자의 기대 종료값 77은 2초 경과와 정상 복귀를 요구한다. 독립 산술 시험은 오류 반환값을 주입해 경계 처리를 검사한다. 실제 POSIX 신호 전달로 대기가 중단되는 경우는 후속 게스트 시험 대상이다.
