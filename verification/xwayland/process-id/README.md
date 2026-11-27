# Native process identity / 네이티브 프로세스 식별자

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. Xwayland uses process identity in its lock and local listener paths. This change covers the single-core AArch64 native FIFO/RR process profile and its optional shell catalog. [Link inventory #471](https://github.com/Romanof123/JNOX/issues/471)

The Rust kernel verifies the scheduler's current owner against the active MMU address space. SVC 201 accepts three zero arguments and returns status in x0, the caller's PID in x1 and its current parent PID in x2. Other arguments return native `EINVAL` and clear both output words. The ordinary C SDK and the graphics Picolibc library implement `getpid()` and `getppid()` through this query. Callers can observe their own identity across scheduling points.

| Process or event | Public identity in this boot |
| --- | --- |
| Packaged slot zero | PID equals the fixed package process count. |
| Packaged slot greater than zero | PID equals its positive slot number, preserving existing child handles from `jnox_spawn` and `jnox_waitpid`. |
| Boot-launched process or standby launched by the kernel | PPID 0 denotes a process with no userspace parent. |
| Shell-launched child | PPID is the shell's public PID while the shell lives. When the shell exits the existing child table marks it orphaned. Its next parent query returns 0. |
| Exit and attempted reuse | The scheduler cannot relaunch a reaped shell slot. A standby has a distinct slot. PID stability therefore extends over the entire process lifetime, and PIDs may recur on a fresh boot. |

A system that adds same-slot relaunch will need a generation-aware public allocator before enabling this contract there. Thread-only and other scheduler adapters have separate syscall dispatchers. This scope supplies numeric identity; group membership, credential checks and signal delivery have their own acceptance under #471.

Run the three-process guest from a source checkout with the existing graphics cross runtime. Set `PATH` to the verified compiler, Cargo and QEMU tools.

```sh
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/process-id/guest.py \
  "$JNOX_WAYLAND_RUNTIME" "$NEW_OUTPUT"
```

The native SDK parent expects PID 3 and PPID 0. It tests invalid query arguments, reaps child PID 1 and observes `EBUSY` on relaunch. The first Picolibc child verifies PID 1 and PPID 3. The second sees PID 2 and PPID 3, then blocks on an empty pipe. The parent's exit closes the last writer and wakes that child. It sees EOF, the same PID 2 and PPID 0. The verifier requires kernel exit rows 80, 81 and 82 for slots 0, 1 and 2 and one pipe block/wake for slot 2. The raw UART remains in the local output directory.

고정 FIFO/RR 네이티브 패키지에서 커널이 스케줄러 소유자와 MMU 주소 공간을 대조한 뒤 호출자 PID와 부모 PID를 반환한다. 0번 슬롯에는 패키지의 프로세스 수를 양수 PID로 배정하고, 다른 슬롯은 기존 생성·회수 API가 반환하는 양수 PID를 보존한다. 부팅 프로세스와 셸 종료 후 고아가 된 자식의 PPID는 0이다. 한 부팅 안에서 셸 자식 슬롯은 한 번만 실행할 수 있다.

세 프로세스 게스트는 공개 PID가 생성·회수 결과와 일치하는지, 종료한 자식의 슬롯 재사용이 거부되는지, 부모 종료로 파이프에서 깨어난 자식이 자기 PID를 유지하면서 부모 PID의 변화를 실제로 관측하는지 검사한다. 위 명령은 시험 절차이며 실행 결과는 별도 증거로 기록한다.
