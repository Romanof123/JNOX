# CHG-338 · Resident Wayland compositor / JNOX 상주 Wayland 합성기

REQ-015/024 · DES-009/013 · VER-015/024. Package the compositor, two client programs and the local socket broker in separate ordinary AArch64 address spaces. Only the compositor owns the framebuffer and input. Clients write pixels into their own shared backing, derive a read-only descriptor, and transfer it over AF_UNIX to the compositor. [Resident window-system acceptance #412](https://github.com/Romanof123/JNOX/issues/412) · [Compositor acceptance #446](https://github.com/Romanof123/JNOX/issues/446) · [Descriptor transfer #444](https://github.com/Romanof123/JNOX/issues/444)

## Protocol and resource ownership

A new connection starts with its own `wl_display` object. Expose `wl_registry`, `wl_compositor` through version 4, `wl_shm` version 1, `xdg_wm_base` version 1 and `wl_seat` through version 4. Bind the lesser of the client and advertised versions. Decode complete little-endian wire messages on this target and queue received descriptors separately in stream order. A single receive can hold part of a header, multiple messages or an FD for a later message. Separate independent connections may use identical object IDs. Verify types, object lifetime, sizes, offsets, stride and formats before use. A protocol violation terminates the offending connection. [Wayland wire format](https://wayland.freedesktop.org/docs/book/Protocol.html) · [Wayland core protocol](https://wayland.freedesktop.org/docs/html/apa.html)

Create a pool by mapping the transferred backing, then create buffers as ranges within that pool. Retain the shared mapping while a live buffer needs it. An initial empty surface commit leads to toplevel and xdg-surface configure events. The client acknowledges the serial before attaching pixels. Maintain pending and committed surface state. Send `wl_buffer.release` once the compositor has finished reading the buffer. Send frame callbacks and retire callback IDs according to their wire lifetime. Keymaps use an actual readable file descriptor and the XKB format. Route pressed and released keys and focus changes to live keyboard objects belonging to the appropriate client.

## Verification

1. Build a Linux reference client with the distribution's unmodified libwayland-client and generated stable xdg-shell bindings. Exercise the same compositor dispatch code through a Linux Unix-socket backend. Record library and XML digests and native/display backend differences.
2. Cold-boot JNOX with the native backend, two independent clients and the AF_UNIX broker. Observe overlap, focus changes, two independent client-rendered buffers, frame callbacks and buffer release. A focused client exits with outstanding work. Capture the surviving surface processing new input. Compare full external frames, private client memory and backing permissions.
3. Track compositor restart and continued headless control separately, including re-binding the endpoint, fresh object namespaces, old FD cleanup and a measured timing bound. The four-role display run does not by itself exercise a fifth independent control workload. Run Xwayland itself with the named X11 client and a native window under #412. Physical input, panel and interference qualification require the selected hardware.

The selected reference XML files are from Ubuntu Jammy packages. SHA-256 for core 1.20.0 is `e20d8e766908e56828b3d39d20968382a3e725cfbca97bbbf0bbfab956cc7455`. SHA-256 for stable xdg-shell from wayland-protocols 1.25 is `637877607a10bcff2f33722d2607ab1edb53ff70325e9df46372c1afd811af43`.

## 한국어 설계와 검증

JNOX 게스트 안에서 화면·입력을 소유한 합성기 한 개, 서로 격리된 앱 두 개, 로컬 소켓 브로커를 실행한다. 앱은 자신이 쓴 픽셀을 읽기 전용 FD로 보내고, 합성기는 버퍼가 가리키는 범위와 권한을 확인한다. 앱마다 객체 ID 공간을 따로 관리한다. 수신 메시지가 나뉘거나 여러 메시지가 붙어 도착해도 FD는 순서대로 올바른 요청에 연결한다.

앱이 초기 설정 이벤트에 응답한 뒤 제출한 버퍼를 화면에 표시하고 읽기가 끝나면 반환을 알린다. 두 창이 겹친 상태에서 포커스를 바꾸고 실제 키를 입력한다. 한 앱이 버퍼를 제출한 직후 종료해도 다른 앱이 계속 화면을 갱신해야 한다. 표준 클라이언트 라이브러리와 호스트에서 확인한 프로토콜 호환, JNOX 게스트의 실행 결과, 합성기 재시작 중 제어 태스크의 연속 실행을 각각 기록한다.
