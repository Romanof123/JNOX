# X11 lock owner lookup / X11 잠금 소유 프로세스 조회

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. Xwayland 24.1.6 `os/utils.c`, `LockServer`, calls `kill(l_pid, 0)` after reading a display's existing lock file. It removes a stale lock on `ESRCH` and treats success or `EPERM` as an existing owner. [OS interfaces #471](https://github.com/Romanof123/JNOX/issues/471) · [Process IDs](../process-id/README.md)

The native and Picolibc C SDKs expose the positive-PID probe through `<signal.h>`. In the fixed FIFO/RR profile SVC 209 validates the caller against both the scheduled owner and active address space. It checks the actual target PID and scheduler state under the single-CPU syscall mask. The shell child table retains existence for an exited child until reaping. The boot package assigns each slot at most one lifetime. [Kernel](../../../kernel/src/native_signal_probe.rs) · [SDK](../../../sdk/native-aarch64/process_signal.inc)

| Request | Result |
| --- | --- |
| Signal 0 to self or from the shell to its own launched child | Zero, preserving `errno`. Ready, running, sleeping, blocked and unreaped shell children retain identity. |
| Existing process outside that authority | `EPERM`. Knowing a numeric PID gives no grant to manage a peer. |
| Missing PID, dormant or dismissed slot, reaped child, or a terminated boot process | `ESRCH`. |
| Valid nonzero signal on an authorized PID or a process group/broadcast selector | `ENOTSUP`. |
| Invalid signal number outside 0–31 or malformed SVC arguments | `EINVAL`. |

Xwayland also calls `sigaction` for real handlers during `OsInit` and sets handlers for hangup and termination in `os/connection.c`. The smart-parent protocol conditionally sends `SIGUSR1`. These require a signal delivery contract, disposition inheritance and safe interrupted frame restoration. The lock probe has no signal queues, handler entry or return path. Running the complete server and observing its shutdown remains separate.

The three-role FIFO guest checks both SDKs. It tests a dormant PID, self, denied parent lookup, malformed arguments and explicit nonzero-signal rejection. The first child retains a pipe writer until exit. The shell observes EOF, probes the zombie, reaps it and checks `ESRCH`. The second child blocks on a pipe. The shell successfully probes the blocked child, exits, and the waking child verifies that the former parent is gone. Expected terminal slot exits are 80, 81 and 82 with one block and wake for the second child. [Parent fixture](parent.c) · [Picolibc child fixture](child.c)

```sh
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/process-id/guest.py \
  "$JNOX_WAYLAND_RUNTIME" "$NEW_OUTPUT" --signal-probe
```

Xwayland는 기존 잠금 파일에서 PID를 읽은 뒤 0번 신호로 소유자를 조회한다. 커널은 호출자의 스케줄러 슬롯과 주소 공간을 먼저 확인한다. 자기 자신과 셸이 기동한 자식은 조회할 수 있고, 권한이 없는 다른 프로세스는 `EPERM`을 반환한다. 미기동·회수된 PID는 `ESRCH`이며, 아직 회수하지 않은 자식은 존재한다. 실제 신호 전달과 등록한 핸들러 실행은 후속 검증 항목이다. 게스트 시험은 파이프 EOF를 이용해 종료·회수 전후를 구별한다.
