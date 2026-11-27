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
