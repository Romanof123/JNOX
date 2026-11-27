# CHG-340 · Xwayland 24.1.6 native port (#448)

REQ-015/024 · DES-009/013 · VER-015/024. The target is a real Xwayland server connected to the guest compositor and serving the Mosaic X11 application. [Port acceptance #448](https://github.com/Romanof123/JNOX/issues/448) · [Guest compositor #446](https://github.com/Romanof123/JNOX/issues/446) · [Guest pointer #451](https://github.com/Romanof123/JNOX/issues/451)

Run `sh verification/xwayland/intake.sh /path/to/cache` on Linux. The script accepts an existing cache, verifies the pinned downloads, extracts the arm64 executable and writes its static ELF report. Debian source version 2:24.1.6-1 records the same upstream archive SHA-256 as X.Org. The retained detached-signature transcript reports both VALIDSIG and EXPKEYSIG for 67DC86F2623FC5FD4BB5225D14706DBE1E4B4540. The reproducible check here pins bytes, and does not authenticate a current signing key.

| Boundary | Port intake observations | First evidence required |
| --- | --- | --- |
| Executable | Debian AArch64 PIE requests /lib/ld-linux-aarch64.so.1 and 17 direct shared objects. kernel/native/elf.rs accepts AArch64 ET_EXEC and only NULL, LOAD and constrained GNU_STACK program headers. | Cross-link an upstream software-rendered Xwayland using JNOX startup, libc/syscall adapters, static target libraries and the native linker layout. Pass the existing native ELF planner before guest packaging. |
| Transport and OS | services/wayland/src/wire.rs handles bounded messages and transferred descriptors. The Debian executable expects Linux/glibc services. The source port needs libc, local sockets, polling, mmap and shared-file semantics usable by real libwayland. | In the guest, start the server, capture Wayland registry exchange and the X11 listening socket, then complete an independent X11 client handshake. |
| Surface | Four globals include compositor v4, shm v1, xdg_wm_base v1 and seat v4. The server allows one surface per client and 320 × 240 pixels of backing storage. Upstream binds wl_output v2+ when available. | Start rootful with an explicit small geometry, capture configure/ack and a committed visible Mosaic window. Trace additional surface requests and output assumptions. |
| Mouse | The seat advertises keyboard only and rejects get_pointer. | Route a real guest pointing device through compositor focus and wl_pointer enter/motion/button coordinates, then observe X11 MotionNotify and ButtonPress/Release and a Mosaic link navigation. |

The 17 direct dependencies are libpixman-1, libXfont2, libXdmcp, libwayland-client, libxcvt, libxshmfence, libdrm, libepoxy, libgbm, libdecor-0, libm, libgcrypt, libtirpc, libXau, libGL, libc and the dynamic loader. Their exact SONAMEs are reported by the intake script. Transitive dependencies are unmeasured.

Use software.ini as a second Meson cross file after the target toolchain file (`--cross-file TOOLCHAIN --cross-file verification/xwayland/software.ini`). It selects upstream software rendering and local X11 and disables optional acceleration, decoration and network authentication. Use only target pkg-config paths. The first port gate is a successful target dependency configure, followed by linking the upstream hw/xwayland/Xwayland executable and reporting its ELF headers and native planner result. The target configure stopped at the first dependency query as recorded below.

## 한국어

Debian 실행 파일은 AArch64 PIE이며 glibc 동적 로더를 포함한 직접 의존성 17개가 있다. 현재 JNOX ELF 계획기는 고정 주소 ET_EXEC를 받는다. software.ini를 적용한 첫 JNOX 대상 구성은 x11 의존성 조회에서 중단됐다. 다음 단계는 필요한 타깃 의존성을 제공한 뒤 원본 서버를 정적 링크하고 게스트에서 실행하는 것이다.

게스트에서는 Wayland 레지스트리 교환, X11 소켓 연결, Mosaic의 첫 화면을 순서대로 확인한다. 현재 클라이언트당 표면은 하나이며 버퍼 용량은 76,800픽셀이다. 좌석은 키보드만 제공한다. 실제 마우스 이동과 클릭이 wl_pointer를 거쳐 X11 이벤트로 전달되고 Mosaic에서 링크가 열리는 장면까지 확인해야 한다.

Sources: [X.Org release archive](https://www.x.org/releases/individual/xserver/), [Debian archive](https://deb.debian.org/debian/pool/main/x/xwayland/). The local source tree includes meson_options.txt and hw/xwayland/xwayland-screen.c.

## Target configure on SDK revision 092a9d6

After the initial intake, the JNOX SDK compiler probe and upstream Meson configure were run separately. On Ubuntu 22.04 use Clang/LLD/LLVM ar 14.0.0, pkg-config 0.29.2, Ninja 1.10.1, Python 3.10.12 and GNU readelf 2.38. Meson 0.61.2-1 was extracted from the Ubuntu package with SHA-256 `54d4a5b9f2c82c37a5ce0bce9027c623aab5505437d87f12fa858ecf32d0b506`, matching the APT index. The installed python3-distutils 3.10.8-1~22.04 and python3-pkg-resources 59.6.0-1.2ubuntu0.22.04.3 satisfied its Python dependencies. To isolate Meson on a matching host:

```sh
apt-get download meson=0.61.2-1
echo '54d4a5b9f2c82c37a5ce0bce9027c623aab5505437d87f12fa858ecf32d0b506  meson_0.61.2-1_all.deb' | sha256sum -c -
dpkg-deb -x meson_0.61.2-1_all.deb meson-tools
export PYTHONPATH="$PWD/meson-tools/usr/lib/python3/dist-packages"
export MESON="$PWD/meson-tools/usr/bin/meson"
sh verification/xwayland/configure.sh /path/to/xwayland-24.1.6 /new/build /target/prefix
```

The SDK size and extrema assertions compiled. Linking with the native startup object produced an AArch64 ET_EXEC at 0x400000 with two PT_LOAD segments. Meson separately compiled its cross sanity probe and tested the target warning flags. It stopped at upstream meson.build:71 on the mandatory x11 pkg-config query. No target library was installed or linked. The source contains only the declaration of x11_dep, and the Debian binary has no direct libX11 SONAME. Before porting all of Xlib, audit indirect header/tool usage and determine whether the configure requirement can be removed in a documented source patch. The unchanged upstream baseline remains reproducible.

The first local attempt stopped at a wrong Meson executable path. The second stopped at a trailing CR on the last shell command. Their original transcripts remain in the intake cache as configure-01.log and configure-02.log. The final run used absolute tool paths and LF checkout. See evidence/target-configure.txt and evidence/sdk-probe.readelf.

SDK 정수형 크기 및 최솟값·최댓값의 정적 검사를 통과했고 AArch64 ET_EXEC 링크에 성공했다. 원본 Xwayland 구성은 meson.build 71행의 JNOX용 x11 의존성에서 정지했다. Xwayland 바이너리 빌드와 게스트 실행은 수행하지 않았다.

## Native SDK work for the upstream server

- The software pixmap path can select `memfd_create`, set a shrink seal before sizing, request a whole-file shared mapping and close the descriptor while that mapping stays alive. The native SDK and kernel implement those operations with a 307,200-byte backing limit. The receive path installs `FD_CLOEXEC` atomically when requested through `MSG_CMSG_CLOEXEC`. Two planned guest configurations exercise the flag-clear and flag-set cases. [Shared-file contract and guest probe](../../examples/native-xwayland-shm/README.md)
- Local Wayland and X11 sockets need a blocking readiness mechanism. The AF_UNIX broker observes pending connections, buffer space, queued bytes, peer exit and invalid descriptors. The native poll probe waits on two endpoints and records a real timeout and wakeup. The public poll scope covers these local socket slots. [Poll contract and guest probe](../../examples/native-fd-poll/README.md)
- The patched server build uses genuine X.Org protocol headers and targets JNOX libraries. The source of the patch, pinned header package and portable pixman build are tracked with their own procedure. [Target dependencies](dependencies.md)

게스트 SDK에 익명 공유 파일, 축소 봉인, 매핑 수명과 소켓 준비 상태 대기를 구현했다. 두 종류의 FD 수신 옵션과 실제 소켓 대기를 시험하도록 프로브를 추가했다. 이 단계의 코드 병합만으로 게스트 시험이나 Xwayland 서버 기동을 승인하지 않는다.
