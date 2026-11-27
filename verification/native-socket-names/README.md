# Native AF_UNIX socket addresses

The XCB 1.17.0 archive used by the integration has SHA-256 `599ebf9996710fea71622e6e184f3a8ad5b43d0e5fa8c4e407123c88a59a6d55`. Its original `src/xcb_auth.c` hashes to `db1923cb8b465e419e1c6ed3c57e40e478c5da30a83ab64f81bc0c5f946cc323`. The upstream helper passes `(int, struct sockaddr *, socklen_t *)` to each query, starting with `sizeof(struct sockaddr) + 108` bytes. It retries at the full returned length if needed. Its auth path tries the peer first, permits a local AF_UNIX fallback, and after finding a record may query the local address again. [Upstream libxcb 1.17.0 archive](https://xorg.freedesktop.org/archive/individual/lib/libxcb-1.17.0.tar.xz)

The probe uses ordinary socket, bind, connect and accept. It compares the listener's pathname, an unnamed connecting endpoint, a named connecting endpoint, and the accepted FD. It forces shorter-than-family, zero, partial-path and full-length results and checks all trailing canary bytes. It queues a data byte, performs another address query and drains the byte. After peer close and object reuse it checks the original address and EOF. The broker tests additionally exercise distinct authenticated owners, datagram reconnect and sequenced-packet sockets.

```sh
# From a full source checkout with the normal native toolchain available:
JNOX_GRAPHICS_RUNTIME=/absolute/path/to/graphics-runtime \
  sh verification/native-socket-names/run.sh /absolute/path/to/new-results
```

Passing the selected runtime builds the same caller through fresh Picolibc socket headers, error translation and transport archive in a private output prefix. Without that environment variable the script builds the native SDK variant. Each guest packages the tested process in slot 0 and its authorized socket broker in slot 1. Required terminal exits are 77 and 0. Each package retains the raw UART, linked images and symbol-filtered QEMU trace. The tests here exercise address reporting. Upstream FamilyLocal authorization additionally requires a configured local hostname and a matching Xauthority record and cookie. Peer credentials and a native socketpair allocator remain separate interfaces.

JNOX 네이티브 SDK와 Picolibc에서 AF_UNIX 소켓의 로컬 주소·상대 주소를 같은 호출 규약으로 읽는다. 이 시험은 리스너·익명 연결·이름을 지정한 연결·accept로 얻은 소켓을 구분하고, 짧거나 0인 버퍼의 길이 반환과 경계 밖 바이트 보존을 검사한다. 상대 종료와 객체 재사용 이후에도 원래 이름과 EOF를 확인한다. 권한이 부여된 호출자의 슬롯에 대해서만 조회한다. XCB의 FamilyLocal 인증에는 실제 호스트 이름 및 일치하는 Xauthority 레코드와 쿠키가 별도로 필요하다.
