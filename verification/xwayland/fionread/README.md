# Native AF_UNIX streams for Xtrans and libX11 (#448)

Xtrans 1.4.0's SocketBytesReadable passes its result pointer as `char *`. On this AArch64 profile Xtrans selects a four-byte `BytesReadable_t`. The SDK copies exactly one C int to the supplied address. The native SDK and Picolibc overlay share a pointer-result declaration for this single request, numbered 0x541b.

Local broker operation 19 uses the existing authenticated BEGIN/COMMIT exchange with zero argument, path and payload. The reply contains the current unread stream payload count (0–64), without advancing the read position or changing queued descriptor rights. A closed peer with retained bytes still reports the full remaining count. Drained EOF, read shutdown and an empty unconnected stream report zero. A listening socket returns EINVAL, an invalid caller-local FD returns EBADF, and datagram, sequenced-packet and IPv4 sockets return ENOTSUP. The veneer rejects a null result pointer with EFAULT and an unknown request with ENOTTY. Error paths leave the caller's result unchanged.

Run the host engine controls with `rustc --edition 2021 --test services/socket/src/local.rs -o "$out/local-tests" && "$out/local-tests"`. They check ownership, repeated reads of the count, a partial receive, queued data after peer closure, end of stream, descriptor retirement and unsupported types.

For the guest, install the updated graphics transport archive into the existing target sysroot with `python3 sdk/native-aarch64/graphics/os/install.py --prefix "$target"`. Use the existing Wayland runtime cross file with its overlay includes and archive link, and the target protocol prefix populated by prepare-protocols.sh (xtrans-dev 1.4.0-1, Debian SHA-256 `45277c51d5d83db351b61859314b59595c9626ac372fbb2fc0d5542e169d9086`).

```sh
python3 verification/xwayland/fionread/package.py "$runtime/jnox.ini" "$protocol_prefix" "$new_out"
timeout 30s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nographic -monitor none -no-reboot -kernel "$new_out/package/kernel.elf" \
    >"$new_out/uart.log" 2>&1 || test "$?" = 124
grep -Fx 'NATIVE EL0 EXIT PID 0 0' "$new_out/uart.log"
grep -Fx 'NATIVE EL0 EXIT PID 1 0' "$new_out/uart.log"
```

The fixture compiles the pinned Xtrans functions `SocketBytesReadable`,
`SocketReadv`, and `SocketWritev` into the native caller. On the integrated
branch the caller and broker both exited 0. The default Rust broker suite
passed all seven tests. These checks exercise the following public API:

```c
#include <sys/uio.h>
struct iovec bytes[] = {{header, header_size}, {payload, payload_size}};
ssize_t n = writev(connected_unix_stream, bytes, 2);
```

`readv` and `writev` support up to 16 vectors and transfer at most 64
stream bytes per call. A caller advances its vectors by the returned byte
count. The guest sends 72 bytes with empty entries between populated ones,
scatters the result across buffers, retries the unsent suffix, and compares
all 72 bytes. It also checks zero-length requests, negative and excessive
counts, total length overflow, a null nonempty buffer, EOF and closed FDs.
Invalid vectors leave queued bytes untouched. For ancillary messages the
existing `sendmsg`/`recvmsg` limit remains eight vectors.

To rebuild the pinned libX11 1.8.10 single-thread, AF_UNIX target from
source, use the runtime and protocol prefixes from the XCB build and the
configured Unix transport directory from the node identity test. Pass
the original libX11 archive recorded in `../xkb-sources.sha256`.

```sh
v=verification/xwayland/fionread
python3 "$v/gate.py" --out "$WORK/x11" --lab "$WORK" --runtime "$RUNTIME" \
    --identity-target "$NODE_TARGET" --protocol-prefix "$PROTOCOL_PREFIX" \
    --unix-build "$CONFIGURED_UNIX_BUILD" --xlib-tar "$LIBX11_TAR"
python3 "$v/archive-guest.py" --build "$WORK/x11/X11-build" --cross "$WORK/x11/cross.ini" \
    --protocol-prefix "$PROTOCOL_PREFIX" --out "$WORK/xlib-guest" --lock "$WORK/run.lock"
```

The second command checks the architecture of every archive member and
links a real public function from the library into an AArch64 guest.
It tests window geometry parsing without opening a display. [Observed build and guest results](result.en-ko.md)

## 한국어

AF_UNIX 스트림의 수신 큐에 남은 바이트 수를 반환한다. 조회는 데이터를 소비하지 않으며 상대방이 종료해도 대기 중인 바이트를 모두 읽을 때까지 수가 유지된다. 브로커는 커널이 확인한 호출자의 서술자 표를 조회한다. 테스트는 부분 읽기 전후, 다른 호출자의 서술자, 닫힌 스트림과 지원하지 않는 소켓 종류를 검사한다. 게스트 호출자는 원본 Xtrans 함수를 사용한다.

통합 브랜치에서 원본 Xtrans의 세 함수로 전송을 시험했다. 72바이트를
여러 버퍼에서 보내고, 수신 측에서 부분 전송과 재시도를 거쳐 원래 순서를
복원했다. 잘못된 벡터가 수신 큐를 소비하지 않는 것도 확인했다. 앱과
브로커는 모두 0으로 종료했고 호스트의 브로커 시험 7개가 통과했다.
위 두 번째 명령은 정적 라이브러리의 대상 아키텍처를 확인하고 공개
`XParseGeometry` 함수를 게스트에서 호출한다.
