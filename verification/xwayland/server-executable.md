# Building the Xwayland 24.1.6 software executable

Run from the published source checkout. Set JNOX_LAB_ROOT to the shared build root and JNOX_INPUT_ROOT to a directory containing all source archives in xkb-sources.sha256 plus xwayland-24.1.6.tar.xz, libxcvt-0.1.3.tar.xz and libmd_1.1.0.orig.tar.xz. The driver checks the pinned hashes, takes the shared run.lock, and requires a fresh output path.

    export JNOX_LAB_ROOT="$LAB_ROOT"
    export JNOX_INPUT_ROOT="$ARCHIVE_CACHE"
    bash verification/xwayland/server-lab.sh "$NEW_EXT4_OUTPUT" HEAD

The default subdirectories of JNOX_LAB_ROOT are graphics-runtime (original graphics runtime), wayland-runtime (derived native OS runtime), wayland (host scanner and target client), and font-prefix (fonts, X.Org protocol headers and xtrans). Set JNOX_GRAPHICS_RUNTIME, JNOX_WAYLAND_RUNTIME, JNOX_WAYLAND_WORK and JNOX_FONT_PREFIX to select their installed locations. Set JNOX_GIT_DIR for an explicit Git object store. The default source is the current checkout at HEAD.

Cross files record absolute installation paths. A relocated profile needs a regenerated manifest for the original graphics cross file, and JNOX_WAYLAND_PROFILE_SHA256 set to the independently verified hash of the relocated derived profile. The default profile hash and binary pins identify the historical build. Host tools are Bash, Git, flock, Clang/LLD, LLVM archive tools, Python, Meson, Ninja, Make, pkg-config, tar, patch, readelf and sha256sum. Set MESON and PYTHONPATH as needed for a standalone host Meson distribution. The driver adds the prefix-local host xcbgen package.

The full chain uses target libXau, generated libxcb, libX11, libxkbfile, libmd for mandatory SHA-1, libXfont2/fontenc/zlib, Pixman, libxcvt, and libwayland-client/libffi. The build preserves the stock Meson X11 dependency, runs the full XKB+Xlib link probe, and allows upstream Ninja to generate the Wayland client protocol code using the native scanner. The pinned protocol metadata supplies xproto, randrproto, renderproto, xextproto, inputproto, kbproto, fontsproto, fixesproto, damageproto, xcmiscproto, bigreqsproto, presentproto, videoproto, compositeproto, recordproto, scrnsaverproto, resourceproto, xineramaproto and xf86vidmodeproto. The software profile disables glamor, GLX, DRM/DRI3, TCP and XDMCP. The existing one-hunk socket patch honors local-only transport.

The archive-only XKB guest passed previously. The complete X11 client transport and server binary still require a new build. libxcb's xcb_conn.c and xcb_util.c include socket/network declarations and use poll/nonblocking I/O. The graphics OS overlay currently covers AF_UNIX, a bounded broker, poll and process-private synchronization. libXau also uses filesystem and process identity calls. The font lsfontdir utility previously failed to link open/stat/fstat, which may affect the complete server. Record the first actual compiler or linker failure and its originating object. Hardware scanout, input, socket namespace creation and runtime limits require guest evidence after static linking.

The [libmd 1.1.0 source archive](https://deb.debian.org/debian/pool/main/libm/libmd/libmd_1.1.0.orig.tar.xz) is pinned by SHA-256 in the driver. The build retains its original COPYING.
