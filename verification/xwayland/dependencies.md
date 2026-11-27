## JNOX server dependency preparation

Use the [Debian xorgproto package](https://deb.debian.org/debian/pool/main/x/xorgproto/x11proto-dev_2024.1-1_all.deb) and the [Debian pixman upstream archive](https://deb.debian.org/debian/pool/main/p/pixman/pixman_0.44.0.orig.tar.gz). The scripts verify their pinned SHA-256 values. Run the following with CACHE containing the downloads and RELEASE containing the original pinned Xwayland tarball.

```sh
sh verification/xwayland/prepare-protocols.sh "$CACHE" "$RELEASE" "$NEW_WORK"
sh verification/xwayland/configure.sh "$NEW_WORK/xwayland-24.1.6" "$NEW_WORK/server-probe" "$NEW_WORK/target"
```

The preparation step stages the architecture-independent upstream headers and 29 original pkg-config files. It rewrites their installation prefix to follow the target metadata directory. The 2021.5 package has Present 1.2. The selected 2024.1 package supplies Present 1.4, as required by Xwayland 24.1.6.

The source patch gates the unused x11 dependency declaration on the JNOX host machine. The patch applies only to an extracted copy. Searching upstream for header names owned by Ubuntu libx11-dev 2:1.7.5-1ubuntu0.4 found X11/Xlib.h directly included only in test/list.c. X11/Xos.h comes from xorgproto. The original Debian Xwayland ELF has no direct libX11 dependency. This gate targets the server executable. Building Xlib-based tests, resolving transitive headers such as those required by libxkbfile, and providing the runtime xkbcomp invocation in xkb/ddxLoad.c remain separate obligations.

The next target-library attempt uses the authentic portable pixman implementation. The following command requires the existing generated cross file pointing to the same prefix. The script requests static linkage and disabled optional features, serially installs the result and inspects the machine type of every archive member. Tool availability, libc headers and unresolved libc/libm symbols must be assessed before claiming a linked Xwayland server.

```sh
sh verification/xwayland/pixman.sh "$CACHE/pixman_0.44.0.orig.tar.gz" "$NEW_WORK/pixman" "$NEW_WORK/server-probe/jnox.ini" "$NEW_WORK/target"
```

The initial protocol-only preparation installed the headers and applied the Xlib declaration patch. The extended preparation also requires the architecture-independent xtrans-dev 1.4.0-1 package. It stages the original C translation material from X11/Xtrans with the real package metadata used by os/xstrans.c. The source itself is compiled inside the server. The JNOX local-only profile omits the TCP transport at compile time. Other systems retain the upstream compile setting.

Xtrans normally creates or inspects the directory for the local X11 socket path. JNOX's SDK does not yet implement the Ubuntu fchown/fchmod pair. Remove the package's HAS_FCHOWN flag, retain the sticky-directory requirement, and select Xtrans's FAIL_HARD behavior on unsafe directories. A target must supply the actual directory and socket semantics (including ownership checks, creation and unlink) or fail opening the listener. The package staging step does not demonstrate a working listener. The guest acceptance requires the authentic server to accept an X11 handshake on that pathname.

XTrans 원본 C 소스를 타깃 접두 경로에 설치한다. X11 소켓 디렉터리 권한 검사와 미충족 시 실패하는 동작을 유지한다. 실제 서버가 게스트에서 소켓을 열고 Mosaic의 연결을 수락하는 단계에서 경로 처리와 접근 권한을 검증한다.

## First target compile with Xtrans

With the pinned 2024.1 protocol package, authentic Xtrans 1.4.0 source and the JNOX server patch, Meson accepted the required X protocol metadata including Present 1.4, found Xtrans, and reached the missing pixman-1 target archive. Pixman 0.44.0 itself completed target configure. Ninja then invoked Clang on upstream pixman.c using the AArch64 JNOX SDK and stopped on the first missing C runtime header, assert.h in pixman-private.h. The attempt produced no pixman archive or Xwayland ELF. [Server configure](evidence/server-with-xtrans.log) · [Pixman configure and compiler output](evidence/pixman-first-compile.log)

Pixman's common implementation also includes stdio, string, float and stdlib declarations, uses heap allocation and arithmetic, and can emit a library constructor. The cross toolchain requests general registers only for the existing integer SDK probe. The native AArch64 vectors save and restore FPCR, FPSR and all 32 Q registers. Linking a native graphics executable still requires an ABI-consistent C runtime and math library, correct initialization and a guest workload proving its computations and state across context switches. Meson's successful thread feature probe alone does not establish that pixman's required thread operations are available.

X.Org 프로토콜과 Xtrans 의존성 조회를 통과했고 원본 pixman.c의 첫 타깃 컴파일까지 도달했다. 공통 C 런타임의 assert.h가 없어 컴파일이 정지했다. 타깃 라이브러리를 링크하고 픽셀 계산을 실제 JNOX 게스트에서 확인하는 단계가 남아 있다.
