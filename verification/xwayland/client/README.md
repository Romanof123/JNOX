# Upstream Wayland client on JNOX · JNOX 게스트의 Wayland 클라이언트 (#448)

An AArch64 QEMU guest ran Wayland 1.23.1 client code against the resident JNOX compositor. The client selected socket broker PID 3, connected a CLOEXEC AF_UNIX stream to the compositor at /wayland-0 and gave the descriptor to wl_display_connect_to_fd. The guest has no inherited display descriptor or runtime-directory requirement. The upstream registry listener received four globals, wl_display_roundtrip returned successfully, and F_GETFD returned EBADF after wl_display_disconnect. All four process slots exited with status zero. [Application UART lines](evidence/registry-uart.txt) · [Package SHA-256](evidence/package.sha256)

| Registry interface | Name | Version |
|---|---:|---:|
| wl_compositor | 1 | 4 |
| wl_shm | 2 | 1 |
| xdg_wm_base | 3 | 1 |
| wl_seat | 4 | 4 |

The tested client source was f16fb53, the image packaging script was 3881fce, and the successful observer was c5a3872. On the first boot the client reported a successful disconnect, but the observer waited for a process exit before sending q. The kernel prints its exit summary after all roles finish. The second boot used the same image and sent q after the app reported disconnect. This release combines the source on parent 9eda83c; the hashes above identify the tested image. The scope is registry discovery and sync. Surface commits, file-descriptor transfer and Xwayland server startup require separate workloads.

## Reproduce on Linux

Start from a fresh native Picolibc/Pixman runtime built at its final absolute path. Install the target socket, poll, pthread and monotonic-clock adapter into that runtime. [Graphics runtime build](../../../sdk/native-aarch64/graphics/README.md) · [Wayland OS adapter](../../../sdk/native-aarch64/graphics/os/README.md)

In the runtime's jnox.ini, prepend -D_GNU_SOURCE and -I followed immediately by the absolute target/include/jnox-wayland directory to c_args. In c_link_args insert the absolute target/lib/libjnox-wayland-os.a path between runtime.o and libc.a. Keep all original target ABI, startup, linker-script and Picolibc options. Set cache, runtime, work and output to absolute paths; work and output must be new directories.

Use Python 3.10+, Meson, Ninja, GNU make, tar, patch, dpkg-deb, pkg-config, Clang/LLVM, GNU readelf, the repository's native Rust/AArch64 toolchain and qemu-system-aarch64. The scanner uses host Expat. The libffi and Wayland archives use the target toolchain. [Input digests](inputs.sha256) · [Build phases](build.py)

```sh
tool=verification/xwayland/client
python3 sdk/native-aarch64/graphics/os/install.py --prefix "$runtime/target"
sh "$tool/fetch.sh" "$cache"
for phase in prepare protocols ffi client; do
    python3 "$tool/build.py" "$phase" "$cache" "$runtime" "$work" || exit 1
done
sh "$tool/registry-guest.sh" "$work/roundtrip.elf" "$output"
```

The observer records the run in the output directory. For public sharing, select the five application lines beginning JNOX_REGISTRY and the package hashes.

## 한국어

AArch64 QEMU 게스트에서 Wayland 1.23.1 클라이언트가 PID 3의 소켓 브로커를 통해 상주 컴포지터의 /wayland-0에 접속했다. 실제 레지스트리 콜백에서 위 네 인터페이스의 이름과 버전을 받았다. 왕복 동기화가 완료됐으며 연결 해제 뒤 F_GETFD가 EBADF를 반환했다. 네 프로세스 모두 종료 코드 0을 기록했다. 첫 부팅에서는 관찰기가 종료 요약을 기다리며 시간 제한에 도달했다. 같은 이미지를 다시 부팅해 연결 해제 직후 q를 보냈을 때 정상 종료를 확인했다.

재현하려면 Linux에서 Picolibc/Pixman 런타임과 소켓·클록 어댑터를 준비하고 위 명령으로 scanner, libffi, libwayland-client와 게스트를 순서대로 빌드한다. 공개 UART에는 애플리케이션이 직접 출력한 다섯 줄을 수록했다. 이 시험의 범위는 레지스트리 탐색, 왕복 동기화 및 연결 해제이다. [게스트 출력](evidence/registry-uart.txt) · [패키지 해시](evidence/package.sha256)

## Output-aware registry check

The merged compositor advertises `wl_output` v3 in addition to the original four globals. The guest script selects `--profile output` and requires all five callbacks with distinct positive registry names. To boot an older four-global package, run `python3 verification/xwayland/client/registry-boot.py PACKAGE NEW_OUTPUT --profile core`. The core profile accepts the four original interfaces and their supported output extension. The separate output protocol fixture checks binding, geometry, mode, scale and surface membership.

### Optional environment-selected connection (#462)

The registry phase relinks the fixture against the installed AArch64 Wayland and libffi archives. Use a graphics runtime built from this revision so its startup code initializes Picolibc's environment. The default link produces `roundtrip.elf` for the supplied-FD route. Adding `--env-connect` produces a separate `roundtrip-env.elf` for the upstream environment-selected route.

```sh
python3 verification/xwayland/client/build.py registry "$inputs" "$runtime" "$work"
python3 verification/xwayland/client/build.py registry "$inputs" "$runtime" "$work" --env-connect
sh verification/xwayland/client/registry-guest.sh "$work/roundtrip.elf" "$fresh_fd_output"
sh verification/xwayland/client/registry-guest.sh "$work/roundtrip-env.elf" "$fresh_env_output" --env-connect
```

Only client slot 1 receives `WAYLAND_DISPLAY=/wayland-0`. The environment variant checks Picolibc getenv, rules out a WAYLAND_SOCKET override, and asks the upstream library to connect to an absent absolute pathname as a negative control. It then calls `wl_display_connect(NULL)`, retrying only readiness errors with 10 ms poll waits. It retrieves the actual connected FD via `wl_display_get_fd`, checks FD_CLOEXEC, observes the same registry globals and roundtrip, and requires EBADF after upstream disconnect. The UART and JSON record the selected path, FD and negative-control errno. Absence of the extra UART receipt prevents accidentally accepting a supplied-FD ELF in environment mode. The five-global results below cover this route.

환경 연결 모드는 슬롯 1의 Picolibc 환경에 설정한 절대 경로를 읽고 실제 upstream wl_display_connect(NULL)를 호출한다. 존재하지 않는 경로의 실패, 실제 FD와 CLOEXEC, 레지스트리 콜백 및 종료 후 EBADF를 검사한다. 기존 FD 전달 모드는 그대로 쓸 수 있다.

## Five-global cold boots · 다섯 전역 객체의 콜드 부팅

The package and ELF hashes below identify the two AArch64 QEMU cold boots. Each received registry names 1–5 with versions 4, 1, 1, 4 and 3 for wl_compositor, wl_shm, xdg_wm_base, wl_seat and wl_output, respectively. Both completed the upstream roundtrip, observed EBADF after disconnect and exited zero in all four process slots. The environment-selected connection used wl_display_connect(NULL) with WAYLAND_DISPLAY=/wayland-0, observed CLOEXEC on FD 64, and received ENOENT (2) for the absent endpoint. The second client used wl_display_connect_to_fd. [Port #448](https://github.com/Romanof123/JNOX/issues/448) · [Environment #462](https://github.com/Romanof123/JNOX/issues/462)

| Boot | Application output | Package hashes |
|---|---|---|
| Environment-selected | [UART](evidence/five-globals/default-display-uart.txt) | [SHA-256](evidence/five-globals/default-display.sha256) |
| Supplied descriptor | [UART](evidence/five-globals/supplied-fd-uart.txt) | [SHA-256](evidence/five-globals/supplied-fd.sha256) |

From the repository root, use the installed target archives, adapter-equipped graphics runtime and pinned cache from the build above. The script rebuilds start.o and runtime.o from the current checkout and rewrites a private copy of the linker options to use them. It links distinct client ELFs and boots each with its matching package and observer options.

```sh
bash verification/xwayland/client/registry-pair.sh "$work" "$runtime" "$cache" "$fresh_output"
```

같은 소스에서 시작 코드 두 개를 다시 빌드하여 별도의 AArch64 ELF 두 개를 연결하고 각각 콜드 부팅했다. 환경변수 방식은 /wayland-0에 연결했고, 없는 경로는 오류 2로 거부했다. 두 방식 모두 wl_output v3를 포함한 다섯 전역 객체를 수신하고 연결 해제 뒤 파일 디스크립터가 닫혔음을 확인했다. 각 부팅의 네 프로세스가 모두 정상 종료했다. 표의 출력과 해시는 두 부팅에서 수집했으며, 앞 절에서 기존 네 전역 객체 결과도 확인할 수 있다.

## Binding the reported output · 출력 객체 바인딩

Two AArch64 QEMU cold boots used separate upstream client ELFs. Each bound the advertised wl_output at version 3, received geometry, mode, scale and done in that order, sent release and completed a display sync. Each repeated the sequence on a fresh binding on the same connection. The callbacks checked the proxy identity, origin (0,0), unspecified physical size, unknown subpixel layout, JNOX/RAM framebuffer strings, normal transform, current and preferred 320×240 mode, unspecified refresh and unit scale. Each client disconnected and all four processes exited zero. Reproduce with the registry-pair.sh command above.

| Connection | Application UART | Package SHA-256 |
|---|---|---|
| Environment-selected | [Default display](evidence/output-bind/default-display-uart.txt) | [Hashes](evidence/output-bind/default-display.sha256) |
| Supplied descriptor | [Connected FD](evidence/output-bind/supplied-fd-uart.txt) | [Hashes](evidence/output-bind/supplied-fd.sha256) |

A separate Linux host run built the resident compositor from the public parent and exercised its Unix socket with the wire fixture. It checked version-specific output events, per-peer surface enter/leave, clipping, release/delete_id, object ID reuse and errors for released or foreign IDs. To repeat from the repository root with Python assertions enabled, run the following. [Host wire fixture](../../wayland/probe-output.py)

```sh
export CARGO_TARGET_DIR="$PWD/target-output-host"
cargo build --offline --release --manifest-path services/wayland/Cargo.toml
python3 verification/wayland/probe-output.py "$CARGO_TARGET_DIR/release/jnox-wayland"
```

AArch64 게스트 두 부팅에서 서로 다른 upstream 클라이언트가 각각 wl_output v3에 두 번 바인딩했다. 각 바인딩에서 geometry, 320×240 mode, scale 1, done을 순서대로 검증하고 release 뒤 동기화를 완료했다. 두 게스트 모두 연결을 닫았으며 각 부팅의 네 프로세스가 정상 종료했다. 위 표는 클라이언트가 출력한 UART와 패키지 해시를 담고 있다.

별도의 Linux 호스트 시험은 같은 컴포지터의 출력 버전별 이벤트, 표면 진입·이탈과 클리핑, 해제된 객체 ID 및 다른 클라이언트 ID의 거부를 검증했다. Xwayland/Mosaic의 화면 출력은 별도 검증 범위이다. [Output #461](https://github.com/Romanof123/JNOX/issues/461) · [Xwayland #448](https://github.com/Romanof123/JNOX/issues/448)
