# Signal delivery to a sleeping child / 대기 중인 자식에게 신호 전달

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. A shell needs to notify an application it launched, including one waiting for a timer. On the fixed-priority shell and shared-FD profiles, an authorized positive signal selects the target's disposition. Signal zero keeps its lifetime and permission query semantics. The shell fixture uses Picolibc's `SIGUSR1` value, 30 on this target. [Process authority](../signal-probe/README.md) · [OS interfaces #471](https://github.com/Romanof123/JNOX/issues/471)

The guest has one CPU. The parent runs at FIFO priority 4 and both children at 6. First, the observer child blocks on an empty pipe. The receiving child installs a `SIGUSR1` handler. It checks that attempts to signal its sibling and parent return `EPERM`, writes a readiness byte, and enters `sleep(15)`. Its higher fixed priority makes the parent resume after the receiver blocks. The shell sends `SIGUSR1` to that child. The handler records the signal once; the resumed sleeper checks `EINTR`, a positive remaining duration and a monotonic return before the original 15-second interval expires. It sends the remaining seconds to the parent.

The parent reaps the receiver and checks that a subsequent send returns `ESRCH`. Its exit releases the observer's pipe, which must wake once with EOF. The verifier requires exits 83, 84 and 85, the kernel's pipe block/wake counter, and the remaining duration (1–15 seconds) in the parent's console receipt. The receipt also detects loss of the child's pipe handle or continuation after handler return. [Parent](parent.c) · [Receiver and observer](child.c)

```sh
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/process-id/guest.py \
  "$JNOX_WAYLAND_RUNTIME" "$NEW_OUTPUT" --signal-delivery
```

Xwayland의 선택적 smart-parent 경로는 자식에서 부모로 신호를 전송하며 별도 권한이 필요하다. 이 시험은 셸이 자신이 기동한 자식에게 `SIGUSR1`을 보내는 경로를 대상으로 한다. 수신 측은 실제 핸들러 진입, 절대 타이머 대기의 조기 복귀, `EINTR`, 남은 초를 검사한다. 정당한 권한이 없는 형제·부모로의 전송은 `EPERM`, 회수된 PID로의 전송은 `ESRCH`를 요구한다. 실제 Xwayland 종료·재기동은 별도로 검증한다.
