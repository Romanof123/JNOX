# JNOX X11 Mosaic

`client.c` is a freestanding AArch64 X11 core client. The window belongs to the process in JNOX slot 0. Its requests reach a separate X server through the JNOX IPv4 service in slot 1 and the QEMU guest's TCP connection to 10.0.2.2. The display server performs rendering and input dispatch.

The window's `WM_NAME` is `JNOX X11 Mosaic`. Its initial interior measures 240 × 160 pixels. On a 24-bit TrueColor display, the following window-relative pixels identify its two states. The client converts the colors through the server's root visual masks.

| Pixel (x, y) | Before first left press | After first left press |
| --- | --- | --- |
| (88, 72) | #182840 | #182840 |
| (32, 32) | #20b8a0 | #20b8a0 |
| (120, 32) | #f4b644 | #f4b644 |
| (128, 110) | #e85c50 | #498cf0 |

Click inside the window to change the bottom-right tile from coral to blue. A second left click inside the window requests destruction. The client waits for the server's `DestroyNotify` and closes the socket. For automated input, use the server's pointer input path, for example an XTEST press and release. The client checks the originating window, button, coordinates and X11 `SendEvent` flag. An `Expose` redraw preserves the current color.

## Build and package

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

## 한국어

JNOX 슬롯 0에서 실행하는 `client.c`는 슬롯 1의 IPv4 서비스로 외부 X 서버에 접속한다. 클라이언트가 창 생성·매핑·그리기를 요청하고, 서버가 해당 창을 표시하고 입력 이벤트를 전달한다. 위 명령으로 독립 실행 ELF 두 개와 부팅 패키지를 만들 수 있다. 매 실행마다 새 16바이트 쿠키를 생성하여 패키지와 격리된 서버의 Xauthority에 같은 값을 설정한다. 쿠키가 들어 있는 패키지, 매니페스트, 원본 캡처는 비공개로 관리한다.

`JNOX X11 Mosaic` 창 안에서 왼쪽 버튼을 누르면 오른쪽 아래 사각형이 산호색에서 파란색으로 바뀐다. 다시 누르면 창이 닫힌다. 위 표의 좌표는 창 왼쪽 위를 원점으로 삼는다. 두 번째 입력에 따른 `DestroyNotify`를 수신한 뒤 게스트 클라이언트와 IPv4 서비스가 각각 종료 코드 0을 반환한다. 이 예제에서 X 서버는 게스트 외부에서 실행한다.

`--local-scanout` 검증에서는 게스트가 외부 X 서버에서 `GetImage` 응답으로 창의 픽셀을 읽고 RGB로 변환한다. 게스트는 320 × 240 QEMU 프레임버퍼의 (40, 40)부터 한 줄씩 써서 창을 표시한다. 검증기는 첫 클릭 전후 두 화면을 각각 서버에서 읽은 픽셀과 비교한다.
