# Signal setup error handling / 신호 설정 오류 처리

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. In the selected Xwayland 24.1.6 source, `os/utils.c:OsSignal` installed a disposition and read the previous handler even when `sigaction` failed. `os/connection.c:InitParentProcess` then attempted to restore that handler. The server uses these operations to discover whether its parent expects a ready notification via SIGUSR1. [Native interfaces #471](https://github.com/Romanof123/JNOX/issues/471)

The port returns `SIG_ERR` on failure. Startup stops if installing a required signal disposition fails, or if the two-step check of the inherited SIGUSR1 disposition fails at either step. The server sets the parent notification flag only after restoration succeeds. The ordinary success path still installs SIGPIPE ignore, the HUP reset callback, and the INT/TERM quit callbacks. The original parent handler remains installed.

On a POSIX build host, run the following command against the actual source directory after the server patches have been applied.

```sh
python3 verification/xwayland/signals/probe.py "$NEW_EXT4_OUTPUT/server/xwayland-24.1.6"
```

The fixture compiles the two functions and four startup registrations extracted from that directory. It injects failures with a deliberately unusable previous-handler result and checks the server's return or fatal path. It also checks preservation of inherited default, ignored and custom handlers. The fixture exercises error handling only. Native `sigaction` and positive signal delivery require per-process disposition storage, supported mask/flags, pending delivery, and the lifecycle of user handlers. The link and guest acceptance remain tracked under #471.

선택한 원본 서버는 신호 설정 실패 후 이전 핸들러를 읽고 복원하려 했다. 수정된 경로는 실패 시 `SIG_ERR`를 반환한다. 부모 통지 판정 중 설치나 복원이 실패하거나 필수 신호 설정에 실패하면 서버 시작을 중단한다. 호스트 시험은 실제 서버 함수에서 각 실패와 정상 복원을 확인한다. 게스트의 신호 전달과 핸들러 복귀는 별도 커널 구현·검증 항목이다.
