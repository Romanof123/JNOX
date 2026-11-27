# JNOX X11 Mosaic

`client.c` is a freestanding AArch64 X11 core client. In its remote profile the window belongs to the process in JNOX slot 0. Its requests reach a separate X server through the JNOX IPv4 service in slot 1 and the QEMU guest's TCP connection to 10.0.2.2. The guest-local profile connects to Xwayland on a native Unix socket. Both profiles use the same window, drawing requests and input handling.

The window's `WM_NAME` is `JNOX X11 Mosaic`. Its initial interior measures 240 × 160 pixels. On a 24-bit TrueColor display, the following window-relative pixels identify its two states. The client converts the colors through the server's root visual masks.

| Pixel (x, y) | Before first left press | After first left press |
| --- | --- | --- |
| (88, 72) | #182840 | #182840 |
| (32, 32) | #20b8a0 | #20b8a0 |
| (120, 32) | #f4b644 | #f4b644 |
| (128, 110) | #e85c50 | #498cf0 |

Click inside the window to change the bottom-right tile from coral to blue. A second left click inside the window requests destruction. The client waits for the server's `DestroyNotify` and closes the socket. For automated input, use the server's pointer input path, for example an XTEST press and release. The client checks the originating window, button, coordinates and X11 `SendEvent` flag. An `Expose` redraw preserves the current color.

## Build and package for a remote server

From the repository root in a Linux build environment, build the client and service. The verifier needs QEMU AArch64, Xvfb and Python Xlib. Give the verifier a fresh output directory. It provisions its own per-run Xauthority, packages the guest, checks both window states and records both guest exits.

```sh
umask 077
tmp=$(mktemp -d)
sh sdk/native-aarch64/build.sh --local-socket examples/native-x11/client.c "$tmp/x11.elf"
sh sdk/native-aarch64/build-rust.sh services/inet/Cargo.toml "$tmp/inet.elf"
/usr/bin/python3 examples/native-x11/verify.py "$tmp/x11.elf" "$tmp/inet.elf" x11-run
```

To check local presentation as well, run the same ELFs with the scanout verifier.

```sh
/usr/bin/python3 examples/native-x11/verify.py --local-scanout "$tmp/x11.elf" "$tmp/inet.elf" x11-scanout-run
```

This mode gives slot 0 the native display service, adds the client argument `scanout` after the cookie and attaches QEMU's ramfb device. After each redraw, the guest sends five X11 `GetImage` requests for consecutive 240 × 32 strips of its window. It checks each reply's sequence, depth, visual and length, converts the returned ZPixmap rows to XRGB8888 and writes them to the local 320 × 240 framebuffer at (40, 40). Each margin is 40 pixels wide. The external X server still draws the window and dispatches button input. The verifier compares both local frames against the independently read server pixels.

For a separately managed X server, set `port` to its private TCP endpoint and provision the same per-run `cookie` in its Xauthority before starting the guest. Package the two ELFs as follows.

```sh
port=46007
cookie=$(python3 -c 'import secrets; print(secrets.token_hex(16))')
python3 tools/build_native_el0.py --out "$tmp/package" --policy fifo \
    --elf "$tmp/x11.elf" --priority 4 --arg "$port" --arg "$cookie" \
    --peer-elf "$tmp/inet.elf" --peer-priority 7 --peer-arg 1 \
    --ipc-route 0:1 --socket-service 1 --net-service 1
```

The two client arguments are a decimal TCP port (1024–65535) and the 32 hex digits of a fresh MIT-MAGIC-COOKIE-1 credential. The packaged argument image, manifest and any full TCP/RAM capture contain the cookie. Keep these private and remove the run-specific package and authorization data after stopping the isolated server. A successful complete interaction produces exit code 0 in both slots. The client reports a rejected or unsupported setup as 104, a disconnected event stream as 106, and an X11 error or unexpected reply as 107. [X11 core protocol](https://www.x.org/releases/current/doc/xproto/x11protocol.pdf)

## Guest Xwayland on display 0

Launch the same source in PID 4 with the single argument `unix`. Mosaic reads the package's kernel nodename and its own read-only `/run/Xauthority` resource. It selects the first exact FamilyLocal (256) record for that nodename, display number `0`, protocol `MIT-MAGIC-COOKIE-1`, and 16-byte data. The four length-prefixed fields use big-endian lengths. A malformed record, including a truncated trailing record, fails before connection. The local build requires the native uname and Xauthority resource interfaces. An older SDK can build the remote profiles and returns status 115 if asked for local mode.

Mosaic connects through broker PID 3 to `/tmp/.X11-unix/X0`. It sleeps between attempts for up to a minute while the listener is absent or is bound before listen. Once connected it uses the ordinary X11 setup and the same TrueColor drawing and event loop. The compositor in PID 0 owns the framebuffer and pointer, original guest Xwayland runs in PID 1, and PID 2 holds the native Wayland client. Give Mosaic only the route `4:3` for socket traffic. Its local invocation takes no display grant, network backend, credential argument or scanout option.

### Integration controls

Prepare the private inputs on the Linux test host. The last command tests the freestanding record reader against truncation, selector mismatches, unknown families, and a valid second record. The directory argument to `prepare` must be new.

```sh
umask 077
work=$(mktemp -d)
profile=examples/native-x11/local_profile.py
python3 "$profile" prepare "$work/authority" --node guest-x11
cc -std=c11 -Wall -Wextra -Werror examples/native-x11/test_xauthority.c -o "$work/read-test"
"$work/read-test" "$work/authority/server.auth" guest-x11
```

The generator writes three binary files with mode 0600 in a 0700 directory. `server.auth` and `client.auth` share a fresh random credential. `wrong.auth` holds a second, independently generated credential. All three match the supplied node, protocol and display. An optional `--client-node` supplies a different public nodename for per-role resource isolation tests. Use the same node on both sides for the Xwayland boots.

1. Build Mosaic with the normal `--local-socket` command above after merging the native identity and boot-resource SDKs. Package the five real ELFs with nodename `guest-x11`, Xwayland's `-auth /run/Xauthority`, and Mosaic's sole argument `unix`. Keep the client and server images identical across both boots. Retain package hashes and the non-secret role assignments.
2. Attach `server.auth` to the server's fw_cfg resource and `wrong.auth` to the client's in the first boot. Observe original Xwayland in PID 1 binding and listening on the pathname, accepting PID 4's connection, then rejecting setup. Status 114 requires a complete X11 11.0 SetupFailed with the pinned server's exact invalid-key reason. A missing listener returns 102, generic or malformed setup returns 104, missing nodename returns 115 and missing or unmatched authority returns 116. Require PID 4 to exit 114 before any Mosaic window is created.
3. In a fresh cold boot, change only the PID 4 input to `client.auth`. Observe Xwayland accept and authenticate Mosaic, create its named 240 × 160 window, commit the coral frame through the compositor, then accept two physical guest pointer presses. Capture blue after the first press and the destroyed window after the second. Keep the cursor clear of the crop for the frame captures. Match the window origin to the actual surface placement. Verify the second press delivered DestroyNotify and PID 4 exited zero.

```sh
python3 "$profile" check-exit wrong "$wrong_run/uart.bin" --private "$work/authority"
python3 "$profile" check-frame coral "$right_run/before.ppm" --origin 32 32
python3 "$profile" check-frame blue "$right_run/after.ppm" --origin 32 32
python3 "$profile" check-exit right "$right_run/uart.bin" --private "$work/authority"
```

The frame checker compares all 38,400 window pixels to the same pattern used by the remote verifier. The exit checker verifies the kernel's PID 4 status and checks the UART for raw or hex-encoded fixture credentials. Keep authority files, fw_cfg contents and full memory captures private. Publish only screened UART, screenshots, input events, object/ELF identities and non-secret trace fields. The source and checks here prepare the guest run. Acceptance requires the original Xwayland listener, role ownership, server setup decision, actual input, and compositor output in the same guest. [Mosaic guest work #465](https://github.com/Romanof123/JNOX/issues/465) · [Xwayland integration #448](https://github.com/Romanof123/JNOX/issues/448)

## 한국어

JNOX 슬롯 0에서 실행하는 `client.c`는 슬롯 1의 IPv4 서비스로 외부 X 서버에 접속한다. 클라이언트가 창 생성·매핑·그리기를 요청하고, 서버가 해당 창을 표시하고 입력 이벤트를 전달한다. 위 명령으로 독립 실행 ELF 두 개와 부팅 패키지를 만들 수 있다. 매 실행마다 새 16바이트 쿠키를 생성하여 패키지와 격리된 서버의 Xauthority에 같은 값을 설정한다. 쿠키가 들어 있는 패키지, 매니페스트, 원본 캡처는 비공개로 관리한다.

`JNOX X11 Mosaic` 창 안에서 왼쪽 버튼을 누르면 오른쪽 아래 사각형이 산호색에서 파란색으로 바뀐다. 다시 누르면 창이 닫힌다. 위 표의 좌표는 창 왼쪽 위를 원점으로 삼는다. 원격 모드에서는 두 번째 입력에 따른 `DestroyNotify`를 수신한 뒤 게스트 클라이언트와 IPv4 서비스가 각각 종료 코드 0을 반환한다.

`--local-scanout` 검증에서는 게스트가 외부 X 서버에서 `GetImage` 응답으로 창의 픽셀을 읽고 RGB로 변환한다. 게스트는 320 × 240 QEMU 프레임버퍼의 (40, 40)부터 한 줄씩 써서 창을 표시한다. 검증기는 첫 클릭 전후 두 화면을 각각 서버에서 읽은 픽셀과 비교한다.

게스트 Xwayland 모드는 PID 4에 인자 `unix` 하나만 전달한다. Mosaic은 커널이 제공한 노드 이름으로 읽기 전용 `/run/Xauthority`의 FamilyLocal 레코드에서 디스플레이 0과 일치하는 16바이트 쿠키를 선택한다. PID 3의 소켓 브로커를 통해 PID 1의 Xwayland가 연 `/tmp/.X11-unix/X0`에 접속한다. PID 0 컴포지터가 프레임버퍼와 포인터 입력을 담당한다. 쿠키는 실행 인자·로그·공개 증거에 넣지 않는다.

같은 ELF로 두 번 부팅하여 검증한다. 첫 부팅은 PID 4에 다른 쿠키를 제공하고 실제 Xwayland의 키 거부 응답과 종료 코드 114를 확인한다. 두 번째 부팅은 일치하는 쿠키로 인증한 뒤 실제 포인터 클릭 전후 240 × 160 전체 픽셀, 두 번째 클릭에 따른 창 제거와 종료 코드 0을 확인한다. 리스너 소유권과 서버의 인증 처리, 실제 게스트 입력 및 화면 증거를 함께 확보해야 통합 검증이 완료된다.
