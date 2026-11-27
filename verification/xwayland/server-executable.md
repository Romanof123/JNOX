# Building the Xwayland 24.1.6 software executable

The first combined build had 29 unresolved imports; the subsequent interface build reduced that count to 18. Integrated revision `f2e4f3d` resolved the required imports and produced a static AArch64 executable. [First link report](server-link.en-ko.md) · [ELF identity and public guest results](public-interface-runs.en-ko.md) · [Native interfaces #471](https://github.com/Romanof123/JNOX/issues/471)

The JNOX Wayland input path uses the fixed wire values for buttons and keys
from `linux/input-event-codes.h`. The builder verifies the numeric UAPI
header from `linux-libc-dev 5.15.0-204.214`. Set `JNOX_INPUT_EVENT_CODES`
to that header if it is installed outside `/usr/include/linux/`.
Wayland events still arrive over the native socket.

JNOX의 입력 경로는 이 헤더의 키·버튼 번호를 사용하고, 실제 이벤트는
네이티브 Wayland 소켓에서 받는다. 별도 위치에 압축을 풀었으면 위 환경
변수로 해당 파일을 지정한다.

Run from the published source checkout. Set JNOX_LAB_ROOT to the shared build root and JNOX_INPUT_ROOT to a directory containing all source archives in xkb-sources.sha256 plus xwayland-24.1.6.tar.xz, libxcvt-0.1.3.tar.xz and libmd_1.1.0.orig.tar.xz. The driver checks the pinned hashes, takes the shared run.lock, and requires a fresh output path.

    export JNOX_LAB_ROOT="$LAB_ROOT"
    export JNOX_INPUT_ROOT="$ARCHIVE_CACHE"
    bash verification/xwayland/server-lab.sh "$NEW_EXT4_OUTPUT" HEAD

The default command uses the original authorization-file and keyboard-map paths. For the guest profile, set `JNOX_NATIVE_XAUTHORITY=1` and pass `--selected-xkm` with the compositor source and the two host packages as shown in the [link recipe](server-link.en-ko.md). The authorization option adds the boot-resource reader as a separate archive. Both configurations verify the rebuilt target graphics adapter in `$NEW_EXT4_OUTPUT/os-prefix/lib/libjnox-wayland-os.a`.

기본 명령은 원본 서버의 인증 파일과 키맵 경로를 선택한다. 게스트용 구성은 `JNOX_NATIVE_XAUTHORITY=1`과 `--selected-xkm`으로 선택한다. 두 구성 모두 같은 대상 그래픽 어댑터의 SHA-256을 확인한다.

The default subdirectories of JNOX_LAB_ROOT are graphics-runtime (original graphics runtime), wayland-runtime (derived native OS runtime), wayland (host scanner and target client), and font-prefix (fonts, X.Org protocol headers and xtrans). Set JNOX_GRAPHICS_RUNTIME, JNOX_WAYLAND_RUNTIME, JNOX_WAYLAND_WORK and JNOX_FONT_PREFIX to select their installed locations. Set JNOX_GIT_DIR for an explicit Git object store. The default source is the current checkout at HEAD.

Cross files record absolute installation paths. A relocated profile needs a regenerated manifest for the original graphics cross file, and JNOX_WAYLAND_PROFILE_SHA256 set to the independently verified hash of the relocated derived profile. The default profile hash and binary pins identify the historical build. Host tools are Bash, Git, flock, Clang/LLD, LLVM archive tools, Python, Meson, Ninja, Make, pkg-config, tar, patch, readelf and sha256sum. Set MESON and PYTHONPATH as needed for a standalone host Meson distribution. The driver adds the prefix-local host xcbgen package.

The full chain uses target libXau, generated libxcb, libX11, libxkbfile, libmd for mandatory SHA-1, libXfont2/fontenc/zlib, Pixman, libxcvt, and libwayland-client/libffi. The build preserves the stock Meson X11 dependency, runs the full XKB+Xlib link probe, and allows upstream Ninja to generate the Wayland client protocol code using the native scanner. The pinned protocol metadata supplies xproto, randrproto, renderproto, xextproto, inputproto, kbproto, fontsproto, fixesproto, damageproto, xcmiscproto, bigreqsproto, presentproto, videoproto, compositeproto, recordproto, scrnsaverproto, resourceproto, xineramaproto and xf86vidmodeproto. The software profile disables glamor, GLX, DRM/DRI3, TCP and XDMCP. The existing one-hunk socket patch honors local-only transport.

The guest XKB probe exercised the upstream keyboard reader. Separate Xtrans and Xlib probes exercised native stream I/O and geometry parsing. Integrated source now links the required filesystem, identity and time interfaces. Display, input and server restart require observations from the running guest server.

The [libmd 1.1.0 source archive](https://deb.debian.org/debian/pool/main/libm/libmd/libmd_1.1.0.orig.tar.xz) is pinned by SHA-256 in the driver. The build retains its original COPYING.

The combined native-Xauthority and selected-XKM build completed the static
X11 dependency chain. Server configuration found X11 1.8.10 and xkbfile
1.1.3, and Ninja began compiling the actual Xwayland executable target.
The local-only port then compiled the original access, authorization, MIT
cookie and connection translation units. The resulting access object
has no unresolved DNS or IP-interface enumeration functions.
[XCB results](evidence/xcb-local/README.md) · [Xlib build and guest](fionread/result.en-ko.md) · [Local host access](server-local-access.md)

키맵과 인증을 함께 선택한 서버 구성이 X11 1.8.10과 xkbfile 1.1.3을
찾아 Xwayland 본체 컴파일에 진입했다. AF_UNIX 전용 이식으로 접근 제어,
인증, MIT 쿠키, 연결 관리 모듈의 오브젝트를 생성했다. 접근 제어
오브젝트의 미해결 심볼에 DNS나 IP 인터페이스 열거 함수는 없었다.

Revision `537cd22` compiled the actual Xwayland startup, input, screen,
window, pixmap and Present objects in the native software configuration.
The configured target ran all independent Ninja steps sequentially. Its
only failed C compilation was `xwayland-shm.c`, where Picolibc lacked
`F_ADD_SEALS` and `F_SEAL_SHRINK`. The existing native SDK and kernel
already implement the shrink seal. The subsequent Picolibc guest test
rejected shrink with `EPERM` and the server's SHM unit compiled.

통합 리비전 `537cd22`에서 실제 원본 서버의 시작·입력·화면·창·Present
모듈이 컴파일됐다. 독립적인 빌드 단계를 직렬로 모두 실행했을 때 남은
C 컴파일 오류는 `xwayland-shm.c`의 축소 방지 seal 선언이었다. 커널이
기능을 제공하며, 후속 게스트 시험에서 공개 C API의 축소 거부를 확인했다.

## Software rendering and DRM selection

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. The build evidence for this change covers the server-side renderer selection. Guest display and input have separate acceptance observations.

The packaged build entry point above applies `software-backend.patch` to the pinned Xwayland 24.1.6 archive (SHA-256 `737e612ca36bbdf415a911644eb7592cf9389846847b47fa46dc705bd754d2d7`) before Meson generates the executable target. The reported compiler error was the common window header’s inclusion of `xf86drm.h`. The screen also embedded DMA-BUF feedback containing a libdrm device pointer, and the common window-buffer module included the GBM header. [X.Org release archive](https://www.x.org/releases/individual/xserver/xwayland-24.1.6.tar.xz) · [Downstream patch](software-backend.patch)

| Meson selection | Source and runtime effect |
| --- | --- |
| Xwayland glamor enabled | Build the existing GBM backend, DMA-BUF feedback and explicit-sync protocol bindings. Bind their globals through the upstream backend. |
| DRM requested, DRI3 enabled, or Xwayland glamor enabled | Build the DRM lease protocol and backend, register the RandR lease callbacks and handle the compositor’s lease-device global. DRM-only lease forwarding remains available with glamor disabled. |
| All three disabled by `software.ini` | Build the original SHM pixmap, window buffer, output and Present units. The server uses `xwl_shm_pixmap_get_wl_buffer` and Wayland buffer release. The RandR core returns `BadMatch` for a lease request when its callbacks are absent. |

For the serialized build, retain `xwayland-config.h`, the Ninja compile and link commands, and the compiler’s dependency records. Check that `xwayland-shm.c`, `xwayland-window-buffers.c` and `xwayland-present.c` produce target objects. Inspect those dependencies for accidental host DRM/GBM headers. The target link must use the SHM and Present objects and omit glamor, DRM lease, DMA-BUF and DRM syncobj backend/protocol objects. Check the positive DRM configuration separately on a target with its real dependencies. Its source graph must still include the selected backend, generated Wayland protocols and RandR lease handlers. A compilation failure in another unit is evidence of that next dependency only.

Runtime acceptance requires the actual guest Xwayland process to create a `wl_shm_pool` and `wl_buffer`, attach it and commit the corresponding surface through the software compositor. Observe compositor buffer release and subsequent reuse. An X11 Present submission and its completion event exercise the Present path. These checks are pending. [Xwayland integration #448](https://github.com/Romanof123/JNOX/issues/448)

기존 전체 빌드 진입점이 압축 해제된 Xwayland 24.1.6에 패치를 적용한다. 세 옵션을 끈 JNOX 설정에서는 공용 창·화면 헤더에서 DRM·GBM 자료형을 제외하고, RandR 임대 콜백을 등록하지 않는다. 실제 `wl_shm` 픽스맵과 Present 제출, 버퍼 반환 처리는 원본 코드를 사용한다. DRM만 활성화한 구성에서도 원래의 임대 기능을 사용할 수 있다.

직렬 빌드에서는 소프트웨어 오브젝트와 최종 링크 입력, 컴파일러가 실제로 사용한 헤더를 확인한다. DRM 활성 구성은 실제 의존성을 갖춘 대상에서 별도로 확인한다. 게스트 수용에는 원본 Xwayland의 SHM 풀과 버퍼 생성, 화면 커밋, 버퍼 반환·재사용, X11 Present 완료 이벤트가 필요하다.
