# AArch64 Xwayland final link / AArch64 Xwayland 최종 링크

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. The selected profile is Xwayland 24.1.6 with native AF_UNIX, required native Xauthority, the compositor's selected keyboard map and the Wayland shared-memory renderer. [Server port #448](https://github.com/Romanof123/JNOX/issues/448) · [Link and OS interface tracking #471](https://github.com/Romanof123/JNOX/issues/471)

| Integrated source | Final action | Distinct undefined symbols |
| --- | --- | ---: |
| `21e1ff8` after the Picolibc seal adapter | All selected C sources compiled. Ninja attempted the server link at step 335 of 335. LLD exited 1. | 34 |
| `f9ef3cf` with target Xfont private dependencies and two Picolibc helpers | A fresh server build used the prepared target dependencies. The final link exited 1. | 29 |
| [Public PR source](https://github.com/Romanof123/JNOX/tree/d74d63b3ad2c2398645b2db394e8f5d1b8bd91c0), clean repeat | The published build entry point rebuilt the X11 dependency chain and compiled all 334 actions preceding the server link. Ninja 335/335 reached LLD and exited 1. The inventory matched the integrated build symbol for symbol. | 29 |

The change resolved `inflate`, `inflateEnd` and `inflateInit2_` from the font loader's `gunzip.o`. Selecting static Xfont pulls its private zlib 1.3.1 dependency. Picolibc's Ryu conversion also required `__ashlti3` and `__lshrti3`. The builder extracts just those two arithmetic objects from Ubuntu's `libgcc-11-dev-arm64-cross` version `11.4.0-1ubuntu1~22.04.3cross1`, verifies each object's SHA-256 and sole external symbol, and checks both objects are AArch64. The default locator is `aarch64-linux-gnu-gcc-11 -print-libgcc-file-name`. Set `JNOX_AARCH64_LIBGCC` to a matching archive installed elsewhere. [Pinned objects](toolchain-shifts.py)

| SDK area | Remaining imports | Selected examples from LLD's references |
| --- | --- | --- |
| Files, descriptors and permissions (14) | `chmod dup2 fchmod fstat fsync link lseek lstat mkdir open rename stat umask unlink` | `LockServer` links and removes its lock. `trans_mkdir` uses directory metadata. The font directory reader uses `fstat`. The log uses `rename` and `fsync`. [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459) |
| Process identity, groups and signals (11) | `getegid geteuid getgid getpgrp getpid getppid getuid kill seteuid setpgid sigaction` | `PrivsElevated` checks process credentials. `OsInit` handles process groups and signals. Listener creation references the process identity. |
| Time and system limits (4) | `clock_getres gettimeofday sleep sysconf` | `GetTimeInMillis` references both clock interfaces, `LockServer` references sleep, and the transport queries a system limit. |

Each row describes a missing public entry point in the selected graphics runtime. For example, the native pipe SDK already exports `dup2`; its descriptor coverage and Picolibc integration need separate testing. A link reference can come from an optional or fallback path. Guest execution determines the path used by the packaged server.

To build, use the dependency locations in [the server build guide](server-executable.md). From the checkout, select the same compositor source, the `x11-xkb-utils_7.7+7_amd64.deb` host tool and `libxkbfile1_1.1.0-1_amd64.deb`. The build verifies their input hashes. After all objects have compiled, replay the generated link command with LLD's diagnostic limit disabled. The report uses a temporary ELF output and prints its own `link_status` field.

```sh
export JNOX_NATIVE_XAUTHORITY=1
bash verification/xwayland/server-lab.sh "$NEW_EXT4_OUTPUT" HEAD --selected-xkm \
  services/wayland/src/input.rs "$XKBCOMP_DEB" "$LIBXKBFILE_DEB"
# Run after the build completes and releases the shared lock.
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/server-link-report.py \
  "$NEW_EXT4_OUTPUT/server/xwayland-build"
```

On the AArch64 guest, the Picolibc seal caller grew the buffer to 4,096 bytes, received `EPERM` on shrink, then verified `EBADF` after close. Its expected exit value was 75. The server-side `xwayland-shm.c` subsequently compiled. Full server startup, Mosaic pixels, keyboard delivery through the running server, buffer release/reuse and restart remain open. [Guest seal test](../../examples/native-xwayland-shm/README.md) · [Window acceptance #448](https://github.com/Romanof123/JNOX/issues/448)

선택한 AArch64 소프트웨어 구성에서 모든 C 소스를 컴파일했다. 최종 링크의 미해결 심볼 34개 중 폰트 압축 해제 3개와 Picolibc 숫자 변환에 필요한 산술 함수 2개를 실제 대상 라이브러리로 해결했다. 재빌드 결과 남은 29개는 파일·권한·서술자 14개, 프로세스·신원·신호 11개, 시간·시스템 한도 4개다. 두 빌드에서 위 진단 명령으로 전체 목록을 다시 확인했고, 새로 생긴 미해결 심볼은 없었다.

공개 PR의 동일한 빌드 소스로 X11 의존성부터 다시 빌드했을 때도 마지막 335번째 단계에서 같은 29개가 나왔다. 파일시스템 팀은 잠금 파일, UNIX 소켓 이름, 폰트 디렉터리, 로그에서 요구하는 권한과 오류 처리를 #459에서 검증한다. 게스트에서는 공개 C API로 공유 버퍼 축소 차단을 확인했다. 남은 OS 인터페이스의 연결·동작 검증, 서버 ELF 생성 및 실제 Mosaic 창·입력·재시작 수용 시험은 #471과 #448에서 추적한다.
