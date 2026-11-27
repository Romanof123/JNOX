# Launching the five-process X11 guest / 다섯 프로세스 X11 게스트 실행

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. This procedure requires a successfully linked static server and its build receipt from the selected XKM and native Xauthority build. The prerequisites include the staged native tmpfs broker with per-slot grants and the explicit UTC boot anchor used for its metadata. [Server and platform interfaces #471](https://github.com/Romanof123/JNOX/issues/471) · [Guest window acceptance #448](https://github.com/Romanof123/JNOX/issues/448) · [Authorization #465](https://github.com/Romanof123/JNOX/issues/465)

### Public source and acceptance status / 공개 소스와 검증 상태

The first integrated Xwayland guest reported `listener-unavailable` during
startup. This draft includes the subsequent bounded listen backlog, protected
X11 socket directory, peer address from accept, descriptor flag support,
current C environment startup, server heap and separate cursor surface.
The five-role server builder pins the measured graphics OS archive
`b4a91dcb62111bc9852b54a29db4bc2c0be17eaa5c176ed7e90cc47c8bde9901`.
The four-role parent retains its own build receipt. The normal build still
requires its declared upstream archives, toolchain and prepared external
prefixes. The source tree does not carry those binary inputs.
[Platform source](https://github.com/Romanof123/JNOX/pull/473)

On the integration source, the native SDK and Picolibc socket cases each
produced slot exits 77 and 0. The directory grant guest produced 88, 88, 88
and 0. Four native compositor host tests passed. These component results are
recorded separately from a complete matching-cookie render and pointer
session, a wrong-cookie rejection on the real server, an independent repeat
from the public source, and physical hardware acceptance. Those guest and
hardware results remain open here.
[Resident acceptance](https://github.com/Romanof123/JNOX/issues/448)

최초 통합 게스트에서 X11 리스너 생성 오류를 관찰했다. 이 초안에는 소켓 대기열,
X11 디렉터리 보호, 연결 수락과 파일 플래그, C 앱 환경 전달, 서버 힙과 별도
커서 서피스가 포함된다. 통합 소스의 SDK와 Picolibc 소켓 게스트는 각각 77·0,
디렉터리 권한 게스트는 88·88·88·0으로 종료했고, 컴포지터 호스트 시험 네 건이
통과했다. 실제 서버의 인증 거부와 정상 렌더링·포인터 입력, 공개 소스에서의
독립 재현, 실보드 검증은 각각 #448에서 추적한다.

| Slot | Program | Arguments and responsibilities |
| --- | --- | --- |
| 0 | Native Wayland compositor | Owns ramfb and virtio tablet. Serves `/wayland-0`. |
| 1 | Xwayland 24.1.6, rootful software renderer | `:0 -auth /run/Xauthority -geometry 320x240 -shm -fp built-ins -terminate`. Its packaged environment supplies `WAYLAND_DISPLAY=/wayland-0`. |
| 2 | Native Wayland Tiles | `pb` selects the normal pointer-aware Tiles client. |
| 3 | Rust socket broker | The five-active packager supplies client mask 23. Slot 1 receives the writable temporary namespace. |
| 4 | Native X11 Mosaic | `unix` connects to the real server's `/tmp/.X11-unix/X0` and reads the private client authority resource. |

The server's DIX startup selects broker slot 3 before creating the X11 listener or calling native libwayland. Xwayland opens the configured Wayland socket itself. The package grants the broker routes and shared-FD operations used by the software renderer. Mosaic cannot write the framebuffer. The rootful Xwayland app ID selects the compositor's primary position (16,24), independently of the order of the Wayland connections. The X11 window at (32,32) therefore begins at (48,56) on ramfb.

The linked server retains its debug information in the server build directory. The packager derives slot 1's runtime file with `llvm-objcopy --strip-debug`. It checks the source SHA-256 from the normal link receipt, the entry, each RX/R/RW segment's bytes, address, file and memory sizes, alignment, the allocated ABI note and the named text/weak symbols with their addresses. Both file hashes and sizes and all three segment hashes are recorded in `server-image.json` and `workload.json`. The native image window and the package file limit keep their original values. To check this step on its own, use `python3 verification/xwayland/resident/runtime_image.py "$SERVER_OUT/server" "$NEW_RUNTIME_ELF"`.

원본 서버 ELF와 디버그 심볼은 빌드 디렉터리에 보관한다. 1번 슬롯에는 디버그 정보를 분리한 실행 파일을 패키징한다. 원본과 실행 파일의 해시를 각각 기록하고, 세 로드 세그먼트의 메모리 배치·권한·내용, 진입점, ABI 노트와 코드 심볼이 일치해야 게스트 패키징을 계속한다.

Run from the native Linux build checkout with the shared serial lock, the dependency inputs from [the server build guide](../server-executable.md), and the pinned host keyboard packages. Each output path must be new. A fixed test UTC value makes startup repeatable. The input seed describes this boot's test time.

```sh
JNOX_NATIVE_XAUTHORITY=1 bash verification/xwayland/server-lab.sh "$SERVER_OUT" HEAD \
  --selected-xkm services/wayland/src/input.rs "$XKBCOMP_DEB" "$LIBXKBFILE_DEB"
python3 verification/xwayland/resident/package.py "$SERVER_OUT/server" "$WORKLOAD" \
  "$JNOX_LAB_ROOT/run.lock" --boot-utc-seconds 2208988800
python3 verification/xwayland/resident/boot.py "$WORKLOAD" "$OBSERVATION" "$JNOX_LAB_ROOT/run.lock"
```

The packager verifies the server's pinned output, selected implementation symbols and each packaged role's ELF identity. A package or loader rejection is a separate result from a link failure. In the matching cold boot, completed startup is demonstrated by the actual client drawing the 240 × 160 coral scene. The observer compares all 38,400 pixels in the Mosaic crop over two observations at least 200 ms apart. It injects a physical guest tablet click through QMP and matches the blue scene. A second click must produce the client's DestroyNotify exit 0. The compositor then receives its normal quit input. The expected slot exits are 0, 0, 72, 0, 0. In the wrong-cookie boot Mosaic must return 114 after parsing Xwayland's rejection. The JSON reports include the last reached stage and available process exits even on a timed-out run.

The two boots use a newly generated local cookie pair and the same kernel. The kernel loads private per-role authority at boot through fw_cfg. Only role/status, input stage, verified pixel counts and the kernel identity go to the observation directory. Temporary credentials, UART, stderr and intermediate scanouts are removed after observation. Server restart, native-client continuity during restart and real-hardware display and input have separate acceptance criteria.

원본 서버가 받은 `WAYLAND_DISPLAY`는 네이티브 합성기의 경로다. 서버는 자신의 프로세스에서 브로커 3을 선택하고 X11 리스너를 생성한다. 서버와 Mosaic의 인증 자료는 게스트 시작 시 배정하며 역할별로 접근을 제한한다. 동일 커널을 두 번 부팅해 잘못된 쿠키의 실제 거부와 정상 쿠키의 전체 화면·입력 경로를 각각 검사한다.

정상 부팅은 첫 산호색 화면의 전체 창 픽셀 일치, 첫 태블릿 클릭 후 파란색 화면, 두 번째 클릭 후 창 파괴 이벤트와 Mosaic 종료까지 요구한다. 패키지나 부팅이 실패하면 도달한 단계와 관찰한 종료값을 기록한다. 이 문서는 실행 절차이며 실제 검증 결과는 #448에서 추적한다.

### Diagnosing a cold boot / 부팅 실패 진단

The five-active kernel announces each packaged image before validation. The
diagnostic receipt matches each banner to its expected hash. It records the
slot under ELF validation, startup stack construction and first successful
entry into the syscall handler. The server's normal stderr has fixed markers
after broker selection, argument processing, OS initialization, listener
creation and output initialization, and immediately before dispatch.
An unexpected EL0 exception includes the scheduled role, exception class and
its saved PC when the address belongs to an executable user page.

A failed observation also samples the stopped vCPU instruction pointer when
QMP is available. The receipt lists every packaged executable segment covering
that address. The five processes have overlapping user virtual addresses, so
these candidates alone cannot identify the interrupted role. Original images
without phase markers can still report recognized Xtrans and Xwayland errors.
To classify an existing UART while it remains in its private run directory,
use `python3 verification/xwayland/resident/diagnostics.py WORKLOAD.json PRIVATE_UART`.
The output contains only predefined fields and category names.

With the FIFO five-active profile, each process now reports its exit status
at the exit syscall. This allows the observer to request compositor shutdown
after Mosaic finishes. The matching case also waits for the kernel's five
terminal exit records and checks that both sources agree. An older kernel
only publishes the terminal records when the whole session has stopped.

진단 결과는 슬롯별 ELF·초기 스택 처리 상태, 첫 시스템 호출, 서버 초기화 단계,
미리 정의한 오류 분류를 기록한다. EL0 예외 시 실행 권한이 확인된 PC와 프로세스를
함께 기록한다. 호스트가 정지시킨 vCPU의 PC는 별도 항목이다. 여러 앱이 동일한
가상주소를 쓰므로 이 값만으로 현재 앱을 특정할 수 없다. 정상 세션에서는 Mosaic가
종료한 직후에만 컴포지터를 닫고, 마지막에 다섯 슬롯의 최종 종료값도 확인한다.

### Rootful software buffers and pointer cursor

Xwayland 24.1.6 creates a cursor wl_surface while binding the seat, before assigning xdg_surface to its distinct root surface. The compositor supports one xdg toplevel and one independent cursor surface per client in either creation order. The cursor uses a bounded 64×64 premultiplied snapshot (16 KiB per client), a pointer-enter serial and a surface-local hotspot. Its buffer releases, frame callbacks, destruction and pixels are independent of the root. A null cursor hides it. A cursor never becomes a pointer hit target.

The advertised globals are wl_compositor v4, wl_shm v1, xdg_wm_base v1, wl_seat v4 and wl_output v3. For the selected geometry an XRGB8888 root at stride 1,280 occupies 307,200 bytes, exactly 75 kernel backing pages. The ARGB8888 cursor and Tiles' 128×96 XRGB8888 surface use additional objects. Four backing objects exist globally and each process has four mapping slots. Two root pixmaps, a live Tiles backing and one cursor consume all four. The client compositor snapshots committed pixels before release. Peak simultaneous allocations and the server's 512 KiB graphics heap require a live run. Mosaic sends X11 drawing requests for a 240×160 window at (32,32); it has a separate address space.

Focused host regression for PM:

```sh
cargo test --manifest-path services/wayland/Cargo.toml root_and_cursor_have_separate_pixels_roles_and_frame_callbacks
```

The test dispatches cursor-first surface creation, a distinct xdg root, stale and valid pointer-enter serials, role collision, cursor buffer attach/commit, frame ownership, alpha and hotspot placement, hide and destroy. Guest acceptance additionally requires actual Xwayland SHM attaches, independent root and cursor completion, real Mosaic pixels and a pointer click while Tiles is connected.
