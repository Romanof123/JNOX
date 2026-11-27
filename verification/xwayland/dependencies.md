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

Only the header installation and source patch were exercised during preparation. The server configure and pixman build require the serial execution gate. The separately acquired Xtrans 1.4.0 package contains C source and Ubuntu feature defines such as HAS_FCHOWN. Its metadata requires a JNOX capability audit before installation.
