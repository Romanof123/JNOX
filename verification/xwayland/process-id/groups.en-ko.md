# Process groups in the fixed AArch64 package / AArch64 프로세스 그룹

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. Xwayland 24.1.6 `OsInit` checks whether `getpgrp()` returns zero before calling `setpgid(0, 0)`. Each boot-launched JNOX process has a positive group ID from its first instruction. The public C APIs are available through the ordinary SDK and Picolibc. [Xwayland link #471](https://github.com/Romanof123/JNOX/issues/471) · [Process identity contract](README.md)

The FIFO/RR package has one process-group namespace. A boot-launched process, including a kernel-launched standby, starts in a group with its own PID. Shell spawn copies the parent's current group before making the child runnable. A member may form a group with its own PID or join an existing group in the package. A group survives its leader's exit while members remain. A shell zombie counts until reaped. The exit of a parent changes the child's PPID and preserves its group.

| Caller operation | Result |
| --- | --- |
| `getpgrp()` | Current positive PGID. |
| `setpgid(0, 0)` or `setpgid(getpid(), getpid())` | Create or select the caller's own group. |
| Join another positive PGID | Succeeds if any present process in the namespace has that PGID. An absent group returns `EPERM`. |
| Change one's shell child | A child first becomes visible after the named ELF is committed. An unreaped child has already executed its program, so its parent's request returns `EACCES`. |
| Target a peer, missing PID or reaped child | `ESRCH`. Negative PID/PGID or malformed service arguments return `EINVAL`. |

SVC 206 validates the fixed scheduler owner and active address space. For query, pass x0–x2 all zero. Success returns x0 = 0 and the group in x1. For set, pass x0 = 1, x1 = PID and x2 = PGID using signed `pid_t` values. Set and failed calls clear x1 and x2. The public SDK translates native errors into its runtime's errno. Sessions with `setsid`, controlling terminals, group-directed signals and pre-exec child control require separate interfaces. [POSIX process-group semantics](https://man7.org/linux/man-pages/man3/setpgid.3p.html)

From a full source checkout run the following with the existing Picolibc runtime. The verifier uses two independent pipe barriers in one guest. The parent joins group 1 while the leader is alive. Its second child inherits that group. Both remain members after the leader is reaped. The parent leaves, rejoins, leaves again and exits. The orphan retains group 1, creates its own group 2 and observes `EPERM` trying to join the now-empty groups. The verifier requires exit codes 90/91/92, one block and one wake for each child. It stores the serial log and result in the fresh local output directory.

```sh
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/process-id/guest.py \
  "$JNOX_WAYLAND_RUNTIME" "$NEW_OUTPUT" --groups
```

부팅 프로세스는 자신의 양수 PID를 그룹 ID로 사용하며, 셸이 생성한 자식은 실행 가능 상태가 되기 전에 부모의 현재 그룹을 상속한다. 그룹장 종료 후에도 구성원이 남으면 그룹을 유지한다. 부모 종료는 자식의 PPID를 변경하고 그룹은 보존한다. 부모가 이미 실행된 자식의 그룹 변경을 요청하면 `EACCES`, 형제나 회수된 자식을 대상으로 하면 `ESRCH`, 소멸된 그룹으로 가입하려 하면 `EPERM`이다.

게스트는 원래 PID 규약과 같은 세 슬롯에서 상속, 자기 그룹 생성, 실제 그룹 가입, 그룹장 회수 후 재가입, 고아 자식의 그룹 유지, 마지막 구성원이 떠난 뒤의 거부를 검사한다. 위 명령의 실행 결과는 별도로 기록한다.
