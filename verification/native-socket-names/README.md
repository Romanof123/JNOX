# Native AF_UNIX socket addresses

The Xwayland 24.1.6 server builds Xtrans 1.4.0 into `os/xstrans.c`. Its `SocketUNIXCreateListener` binds the counted pathname and requests a backlog of at least 128. `SocketUNIXAccept` supplies both address and length pointers. Once it accepts a client, `os/connection.c` calls `_XSERVTransSetOption(TRANS_NONBLOCKING, 1)`, which reads and sets the descriptor status flags. For an unnamed client the accepted peer address contains only AF_UNIX. The listen, accept and fcntl contracts are separate from creating the server-owned directory.

The guest controls request backlogs 128 and 65536, fill the two-entry queue, observe EAGAIN, accept one connection, refill it and read three different FIFO bytes. They exercise full unnamed and truncated named peer addresses, a null length pointer before consuming a connection, and the accepted descriptor’s `F_GETFL` / `F_SETFL` behavior with an actual empty read. Success for this probe is PID 0 exit 77 on both SDK variants. Original-server acceptance also requires the guest Xwayland process to bind, listen and accept Mosaic. The socket fixtures do not measure a server frame.

Xwayland는 최소 128의 backlog를 요청하고 accept에 주소·길이 버퍼를 넘긴다. 소켓 서비스의 실제 대기열은 두 칸이며, 시험은 세 번째 연결의 EAGAIN과 accept 이후 재연결, 입력 순서를 확인한다. 수락한 소켓에 비차단 모드를 설정한 뒤 빈 소켓에서 EAGAIN을 확인한다. 원본 게스트 Xwayland가 직접 Mosaic 연결을 수락했는지는 통합 실행에서 확인한다.

The XCB 1.17.0 archive used by the integration has SHA-256 `599ebf9996710fea71622e6e184f3a8ad5b43d0e5fa8c4e407123c88a59a6d55`. Its original `src/xcb_auth.c` hashes to `db1923cb8b465e419e1c6ed3c57e40e478c5da30a83ab64f81bc0c5f946cc323`. The upstream helper passes `(int, struct sockaddr *, socklen_t *)` to each query, starting with `sizeof(struct sockaddr) + 108` bytes. It retries at the full returned length if needed. Its auth path tries the peer first, permits a local AF_UNIX fallback, and after finding a record may query the local address again. [Upstream libxcb 1.17.0 archive](https://xorg.freedesktop.org/archive/individual/lib/libxcb-1.17.0.tar.xz)

The probe uses ordinary socket, bind, connect and accept. It compares the listener's pathname, an unnamed connecting endpoint, a named connecting endpoint, and the accepted FD. It forces shorter-than-family, zero, partial-path and full-length results and checks all trailing canary bytes. It queues a data byte, performs another address query and drains the byte. After peer close and object reuse it checks the original address and EOF. The broker tests additionally exercise distinct authenticated owners, datagram reconnect and sequenced-packet sockets.

```sh
# From a full source checkout with the normal native toolchain available:
JNOX_GRAPHICS_RUNTIME=/absolute/path/to/graphics-runtime \
  sh verification/native-socket-names/run.sh /absolute/path/to/new-results
```

Passing the selected runtime builds the same caller through fresh Picolibc socket headers, error translation and transport archive in a private output prefix. Without that environment variable the script builds the native SDK variant. Each guest packages the tested process in slot 0 and its authorized socket broker in slot 1. Required terminal exits are 77 and 0. Each package retains the raw UART, linked images and symbol-filtered QEMU trace. The tests here exercise address reporting. Upstream FamilyLocal authorization additionally requires a configured local hostname and a matching Xauthority record and cookie. Peer credentials and a native socketpair allocator remain separate interfaces.

JNOX 네이티브 SDK와 Picolibc에서 AF_UNIX 소켓의 로컬 주소·상대 주소를 같은 호출 규약으로 읽는다. 이 시험은 리스너·익명 연결·이름을 지정한 연결·accept로 얻은 소켓을 구분하고, 짧거나 0인 버퍼의 길이 반환과 경계 밖 바이트 보존을 검사한다. 상대 종료와 객체 재사용 이후에도 원래 이름과 EOF를 확인한다. 권한이 부여된 호출자의 슬롯에 대해서만 조회한다. XCB의 FamilyLocal 인증에는 실제 호스트 이름 및 일치하는 Xauthority 레코드와 쿠키가 별도로 필요하다.
