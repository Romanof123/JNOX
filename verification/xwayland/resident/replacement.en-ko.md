# Replace the guest Xwayland process / 게스트 Xwayland 프로세스 교체

Proposed extension to CHG-340, REQ-015/024, DES-009/013 and VER-015/024. The acceptance target is one recovery in one AArch64 QEMU boot. The first Xwayland process must already have served an authenticated Mosaic window alongside Tiles. The running compositor, Tiles, socket broker and Mosaic process must continue through the replacement. [Guest Xwayland #448](https://github.com/Romanof123/JNOX/issues/448) · [Resident package](package.py) · [Compositor recovery #449](https://github.com/Romanof123/JNOX/issues/449)

## Boot and ownership

| Slot | Process | Lifetime and rights |
| --- | --- | --- |
| 0 | Wayland compositor | Retains its address space, Wayland listener, scanout and input for the whole boot. |
| 1 | Primary Xwayland | Starts with the active server's X11 listener, tmpfs and authority-resource permissions. |
| 2 | Tiles | Keeps its Wayland connection, window and input state. |
| 3 | Socket and tmpfs broker | Remains live and serializes socket, directory and file-description ownership. |
| 4 | Mosaic | Keeps its process and application state. Opens a fresh X11 connection after disconnection. |
| 5 | Standby Xwayland | Starts dormant. Loads the same verified server ELF in its own address space and gets the active server permissions on the one authorized transfer. |

Add a separately selected six-slot profile. Preserve the existing five-role package and its regression results. Keep the standby's ELF identity, argv, environment, unique public PID and ASID in the manifest. Sharing the immutable ELF input is sufficient; each process needs private writable segments, stack, heap and mappings. Check the actual kernel image and both process layouts against the guest RAM map. The packager, kernel build selection, fixed scheduler, slot arrays, IPC masks, broker and runtime profile validators currently cap this workload at five. The existing compositor standby gives slot 2 the display lease after slot 0 exits. Here slot 0 retains that lease. [Packager](../../../tools/build_native_el0.py) · [Process memory](../../../kernel/src/native_el0_mmu.rs) · [Existing display handoff](../../../kernel/src/native_uart_standby.rs)

## Retirement and launch

1. Arm one replacement for this package. For the first planned rotation, Mosaic closes the first authenticated session after its painted window is observed. The primary runs upstream `-terminate`. Record EL1 process exit. For unexpected termination, use a separately selected, locally controlled failure after the first frame. Ordinary shutdown outside an armed rotation dismisses the dormant standby. A failure of the standby exhausts this policy.
2. Commit primary retirement before exposing the successor. Cancel its pending calls, make its positive PID return `ESRCH` on an existence probe, revoke its direct mappings/descriptors and refuse further execution in its address space. The live broker receives the kernel-authenticated death of slot 1, removes its sockets, staged exchanges and open file descriptions, and discards queued rights released by socket teardown. References belonging to surviving clients retain their existing semantics. [IPC boundary](../../../kernel/src/native_sync_ipc.rs) · [Shared descriptor teardown](../../../kernel/src/native_shared_fd.rs)
3. Add a one-shot acknowledgement from broker slot 3 after the cleanup transaction commits. Bind it to this primary, successor and pending retirement. Handle a repeated acknowledgement idempotently. A stale, wrong-role or premature notification must never launch another process. Change the broker's X11 directory binding grant and tmpfs access to the designated successor, and enable its server-authority resource. Before the successor runs, the old listener must reject new connections. Once cleanup is acknowledged, admit slot 5 at its original entry with a clean frame. A broker failure leaves the standby dormant. [Broker lifecycle](../../../services/socket/src/main.rs) · [Directory grant](../../../services/socket/src/local.rs) · [Role-bound resource](../../../kernel/src/native_display_authority.rs)
4. Keep the mount and its unrelated files. A fault can leave an X lock file because descriptor cleanup preserves linked names. Exercise upstream stale-lock handling with the retired positive PID, then recreation of the listener at `/tmp/.X11-unix/X0`. An occupied lock belonging to a live process must still prevent takeover. Record removal of a stale temporary lock separately if the chosen failure point creates one. [Volatile mount](../../../services/socket/src/tmpfs.rs)
5. Give Mosaic an opt-in reconnection mode. On the planned rotation, wait for confirmed retirement or successor admission before retrying the same socket path. On a fault, observe EOF or the documented connection-termination error. Never reinterpret a malformed X11 message, authentication failure or partial setup as a successful reconnect. Bound retries by monotonic time. Reopen the authority resource, complete a second X11 SetupSuccess and recreate window and graphics resources from the new setup. Track connection instance as well as XID since a new server may allocate the same numerical XID. [Mosaic](../../../examples/native-x11/client.c)

The immediate implementation dependency is the broker acknowledgement and transfer of the single X11 writer. The existing broker clears a dead client's live bit, `Mount::exit` releases handles, and the directory writer remains fixed at initialization. The IPC model's `restart` preserves token order and checks unconsumed death notifications. Receipt of a death does not by itself attest that the broker has finished releasing objects and names. [Endpoint model](../../../kernel/src/sync_ipc.rs)

## Guest acceptance

Record terminal and protocol outcomes with monotonic ordering. Include both server PIDs, distinct address-space identities, the same server ELF digest, exactly one successor entry, the original Mosaic PID, and the same compositor, broker and Tiles identities throughout. Correlate each fresh X11 SetupSuccess with its actual process-owned listener and an observed Mosaic frame. Use a visibly different Mosaic state after reconnect and verify real input reaches it. Show Tiles retaining its existing Wayland connection and responding to input across the gap. Measure elapsed time from primary retirement to the first replacement frame. A timing limit requires a separately allocated requirement.

Run planned rotation, unexpected primary loss with stale lock, clean unarmed shutdown, an occupied live lock, denied/replayed acknowledgement and exhausted replacement as distinct controls. Verify unauthorized roles cannot bind the X11 path or read the server's credentials. Capture metadata, pass/fail and scene pixels without credential bytes. Existing compositor-restart measurements and a fresh DIX server generation cover different lifecycles. Physical-board acceptance remains separate.

## 한국어

한 번 부팅한 JNOX에서 주 Xwayland가 실제 종료한 뒤 같은 ELF의 예비 Xwayland가 다른 PID와 주소 공간에서 시작해야 한다. 합성기 0, Tiles 2, 브로커 3, Mosaic 4는 계속 실행한다. 기존 슬롯 1을 주 서버로 두고, 새 선택 프로필에만 휴면 슬롯 5를 추가한다. 현재의 5개 역할은 모두 사용 중이다. 기존 합성기 복구 정책은 합성기와 디스플레이 권한을 교체하므로 이 시험에 그대로 적용할 수 없다.

첫 구현 항목은 브로커의 정리 완료 통지와 X11 이름·쓰기 권한 인계다. 커널은 주 서버의 실행을 종료하고 매핑·직접 FD를 회수한다. 브로커는 인증된 종료 통지를 받은 뒤 소켓, 대기 중인 요청과 보유 권리를 정리한다. 정리된 슬롯과 예비 슬롯이 일치할 때 한 번만 기동을 허용한다. 관련 없는 파일, Tiles 연결, 합성기의 입력·출력 권한은 계속 유지한다. 정상 종료 뒤와 고장으로 잠금 파일이 남은 경우를 각각 시험한다. 살아 있는 다른 프로세스의 잠금은 유효해야 한다.

Mosaic는 동일 프로세스에서 원래 표시 주소에 다시 연결하고 인증·X11 설정·창 생성을 새로 수행한다. 동일 XID가 재사용될 수 있으므로 연결 세대, 서버 PID, 관측한 화면을 함께 기록한다. Tiles의 기존 연결과 입력 응답, 교체 후 Mosaic 픽셀과 입력을 검증한다. 정상 회전, 예기치 않은 종료, 승인되지 않은 기동, 중복 통지, 예비 서버 재고장을 분리한다. 이 문서는 설계이며 이 프로필의 빌드·게스트 시험 결과는 아직 없다.
