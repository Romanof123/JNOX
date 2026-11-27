# Cross-built XCB and libX11 configure

The local-only XCB 1.17.0 cross-build installed its static libraries, headers
and pkg-config files. The core archive hash is recorded in `xcb.sha256`,
with its members in `xcb-members.txt`. XCB's protocol generator ran on the
build host. Clang 14 targeted aarch64-none-elf with Picolibc. GNU make and
Ninja used a single job. The Wayland scanner, protocol, font, libffi and
runtime pins are documented in `../../server-lab.sh`.

Run the address controls in `verification/native-socket-names/run.sh` with the
pinned graphics runtime. From the checkout root run
`bash verification/xwayland/server-lab.sh "$NEW_EXT4_OUTPUT" HEAD` using the
environment and archive cache in `verification/xwayland/server-executable.md`.
The driver rebuilds the native adapter in an isolated prefix and checks its
digest against the guest-tested archive. The libxcb and settings hashes identify
the original run; absolute paths are embedded in the original build settings.
application-exits.txt contains only exit lines emitted on the normal serial
console by the kernel when these programs returned. The host condition fixture
passed. All six broker unit cases passed. Each of the SDK and Picolibc guest
programs exited 77 and its broker exited 0.

LibX11's configure probes malloc(0), realloc(nonnull,0), and calloc(0,10).
The ordinary target caller in malloc-profile.c exited 66 on the single-process
virt/FIFO guest. Bit 1 records the null realloc result. Configure consequently
used --enable-malloc0returnsnull and completed. Compilation reached
src/xlibi18n/xim_trans.c, where Xtransint.h requires sys/utsname.h.

In this pinned Xtrans source, TRANS(GetHostname) calls uname, reads only
nodename, and truncates and terminates the copy. It ignores uname's return.
Its alternate gethostname branch is selected at compile time. The optional
local transport HostReallyLocal checks uname's status and nodename match,
then tries gethostname. Both need one consistent, configured guest node name.
The local auth cookie and Xauthority database remain separate requirements.
Archive creation establishes no server executable or X11 client exchange.

## 한국어

XCB 1.17.0은 실제 AArch64 정적 라이브러리, 헤더와 pkg-config 파일까지
설치했다. 호스트 조건 변수 시험과 브로커 단위 시험 6개가 통과했다.
네이티브 SDK와 Picolibc 게스트에서 각각 호출자 77, 브로커 0 종료를 확인했다.
게스트 할당기 시험은 66을 반환했고 realloc(p,0)의 NULL 결과를 반영한
옵션으로 libX11 configure가 완료됐다. 다음 빌드 오류는 Xtrans의
sys/utsname.h 요구다. GetHostname에서는 uname 결과의 nodename만 사용하며
호출 실패를 확인하지 않는다. 별도 로컬 전송 경로는 nodename을 비교한 뒤
gethostname도 호출한다. 두 API가 같은 게스트 이름을 제공해야 한다.
Xwayland 실행 파일과 실제 X11 통신은 후속 검증 대상이다.
