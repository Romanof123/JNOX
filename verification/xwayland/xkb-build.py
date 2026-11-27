#!/usr/bin/env python3
"""Build original X.Org target sources. All paths refer to Linux build-host paths."""
import argparse
import ast
import configparser
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
from server_receipt import verify as verify_server

HERE = Path(__file__).resolve().parent


def run(*args, cwd=None, output=False):
    cmd = list(map(str, args))
    print(shlex.join(cmd), flush=True)
    return subprocess.run(cmd, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if output else None).stdout


def source_inputs(cache, out):
    for line in (HERE / "xkb-sources.sha256").read_text().splitlines():
        expected, name = line.split()
        archive = cache / name
        if hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
            raise ValueError("wrong source hash for " + name)
        run("tar", "xf", archive, "-C", out)


def check_archive(path):
    listing = run("readelf", "-hW", path, output=True)
    machines = re.findall(r"^\s*Machine:\s*(.*?)\s*$", listing, re.M)
    if not machines or set(machines) != {"AArch64"}:
        raise ValueError(f"non-target or empty archive: {path}")
    return len(machines)


def probe(out, cc, flags, libs, ld, full):
    name = "probe-x11" if full else "probe-archive"
    target = out / (name + ".elf")
    run(*cc, *flags, *(["-DJNOX_XKB_WITH_X11"] if full else []), HERE / "xkb-link.c",
        *libs, "-Wl,-Map," + str(out / (name + ".map")), *ld, "-o", target)
    headers = run("readelf", "-hlW", target, output=True)
    if "AArch64" not in headers or re.search(r"^\s*(?:INTERP|DYNAMIC|TLS)\s", headers, re.M):
        raise ValueError("target probe has wrong machine, interpreter, dynamic segment or TLS")
    symbols = run("llvm-nm", "--defined-only", target, output=True)
    for name in ["XkbRF_Create", "XkbRF_Free", "XkbModIndexText"] + (["XkbLibraryVersion"] if full else []):
        if not re.search(r"\bT " + name + r"$", symbols, re.M):
            raise ValueError("missing linked target implementation: " + name)
    if run("llvm-nm", "--undefined-only", target, output=True).strip():
        raise ValueError("unresolved target probe symbol")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cache", "runtime", "protocols", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--server", type=Path, help="pinned Xwayland 24.1.6 release archive for full dependency build and configure")
    parser.add_argument("--wayland-work", type=Path,
                        help="for combined server configure, use this completed client build's target libraries and host scanner")
    parser.add_argument("--runtime-profile-sha256", help="digest of a separately verified derived cross file")
    parser.add_argument("--build-executable", action="store_true",
                        help="build the production software Xwayland binary after the full X11 chain")
    parser.add_argument("--native-xauthority", action="store_true",
                        help="require the immutable native Xauthority resource for every client")
    parser.add_argument("--xcvt-prefix", type=Path)
    parser.add_argument("--libmd-source", type=Path)
    parser.add_argument("--os-prefix", type=Path, help="Picolibc target prefix containing the native Wayland OS overlay")
    parser.add_argument("--selected-xkm", nargs=3, type=Path,
                        metavar=("COMPOSITOR_INPUT_RS", "XKBCOMP_DEB", "LIBXKBFILE_DEB"),
                        help="embed the pinned compositor map in the native server")
    args = parser.parse_args()
    if args.selected_xkm and not args.build_executable:
        parser.error("--selected-xkm requires --build-executable")
    if args.native_xauthority and not args.build_executable:
        parser.error("--native-xauthority requires --build-executable")
    if args.build_executable and not all((args.server, args.wayland_work, args.xcvt_prefix,
                                          args.libmd_source, args.os_prefix)):
        parser.error("executable build requires server, wayland-work, xcvt-prefix, libmd-source and os-prefix")
    if args.wayland_work and not args.server:
        parser.error("--wayland-work requires --server")
    out, runtime, protocols = [p.resolve() for p in (args.out, args.runtime, args.protocols)]
    prefix = out / "target"
    out.mkdir(parents=True, exist_ok=False)
    prefix.mkdir()
    source_inputs(args.cache.resolve(), out)
    rtcfg = configparser.ConfigParser(interpolation=None)
    rtcfg.read(runtime / "jnox.ini")
    if rtcfg["host_machine"]["system"] != "'jnox'":
        raise ValueError("expected JNOX graphics runtime cross file")
    manifest = json.loads((runtime / "manifest.json").read_text())
    for rel in ("jnox.ini", "target/lib/libc.a", "target/lib/libpixman-1.a"):
        expected = manifest["artifacts"][rel]
        if rel == "jnox.ini" and args.runtime_profile_sha256:
            expected = args.runtime_profile_sha256
        if hashlib.sha256((runtime / rel).read_bytes()).hexdigest() != expected:
            raise ValueError("graphics runtime input changed: " + rel)
    cc = ast.literal_eval(rtcfg["binaries"]["c"])
    flags = ast.literal_eval(rtcfg["built-in options"]["c_args"])
    ld = ast.literal_eval(rtcfg["built-in options"]["c_link_args"])
    includes = ["-I" + str(prefix / "include"), "-I" + str(protocols / "include")]
    flags = includes + flags
    wayland = args.wayland_work.resolve() if args.wayland_work else None
    if wayland:
        for lib in ("libffi.a", "libwayland-client.a"):
            check_archive(wayland / "target/lib" / lib)
        check_archive(protocols / "lib/libXfont2.a")
    for key in list(os.environ):
        if key.startswith(("PKG_CONFIG_", "ac_cv_")) or key in (
                "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "LIBRARY_PATH", "CONFIG_SITE",
                "CFLAGS", "CPPFLAGS", "LDFLAGS", "LIBS", "DESTDIR", "SDKROOT"):
            os.environ.pop(key)
    roots = [prefix, protocols, runtime / "target"]
    if args.build_executable:
        os_prefix = args.os_prefix.resolve()
        os_archive = os_prefix / "lib/libjnox-wayland-os.a"
        sdk = HERE.parents[1] / "sdk/native-aarch64"
        for name, src in (("start.o", "start.S"), ("runtime.o", "runtime.c")):
            # The full server uses this revision's startup ABI, including envp.
            # Archived graphics libraries predate the packaged environment.
            matches = [i for i, arg in enumerate(ld) if Path(arg).name == name]
            if len(matches) != 1:
                raise ValueError("expected one graphics startup input: " + name)
            obj = out / name
            run(*cc, "-iquote", sdk / "include", *flags, "-Os", "-c",
                sdk / "graphics" / src, "-o", obj)
            ld[matches[0]] = str(obj)
        xcvt = args.xcvt_prefix.resolve()
        check_archive(os_archive)
        check_archive(xcvt / "lib/libxcvt.a")
        roots.append(xcvt)
        # time.h uses include_next to reach the real Picolibc header. A
        # second copy of the OS overlay would stop at its own include guard.
        old_overlay = "-I" + str(runtime / "target/include/jnox-wayland")
        flags = ["-I" + str(os_prefix / "include/jnox-wayland"),
                 *(flag for flag in flags if flag != old_overlay)]
        # The base graphics profile embeds its older OS archive by absolute path.
        ld = [str(os_archive) if Path(arg).name == "libjnox-wayland-os.a" else arg for arg in ld]
        ld = ["-L" + str(prefix / "lib"), "-L" + str(runtime / "target/lib"),
              "-L" + str(protocols / "lib"), "-L" + str(os_prefix / "lib"),
              str(os_archive), *ld]
    if wayland:
        roots.append(wayland / "target")
    pcdirs = [str(p / sub) for p in roots for sub in ("lib/pkgconfig", "share/pkgconfig")]
    os.environ.update(PKG_CONFIG_LIBDIR=os.pathsep.join(pcdirs), PKG_CONFIG_PATH="",
                      CONFIG_SITE="/dev/null", LC_ALL="C", SOURCE_DATE_EPOCH="1745997199")
    for dep in ("xproto", "kbproto", "xtrans", "pixman-1"):
        run("pkg-config", "--exists", dep)
    xkb = out / "libxkbfile-1.1.3"
    x11 = out / "libX11-1.8.10"
    # Build the entire upstream translation-unit list without publishing a .pc.
    # The narrow probe links only functions whose closure needs no X11 transport.
    declarations = (xkb / "src/meson.build").read_text()
    source_list = re.search(r"libxkbfile_sources\s*=\s*\[(.*?)\]", declarations, re.S)
    sources = re.findall(r"'([^']+\.c)'", source_list.group(1))
    if len(sources) != 13 or len(set(sources)) != len(sources):
        raise ValueError("unexpected upstream source list")
    rawflags = [*flags, "-I" + str(x11 / "include"), "-I" + str(xkb / "include"),
                "-I" + str(xkb / "include/X11/extensions"), "-D_DEFAULT_SOURCE", "-Os"]
    objects = []
    for src in sources:
        obj = out / (src + ".o")
        run(*cc, *rawflags, "-c", xkb / "src" / src, "-o", obj)
        objects.append(obj)
    archive = out / "libxkbfile.a"
    run("llvm-ar", "rcsD", archive, *objects)
    if check_archive(archive) != len(objects):
        raise ValueError("missing original translation unit")
    probe(out, cc, rawflags, [archive], ld, False)
    if args.server is None:
        return

    os.environ.update(CC=shlex.join(cc), CFLAGS=shlex.join(flags + ["-Os", "-D_DEFAULT_SOURCE"]),
                      AR="llvm-ar", RANLIB="llvm-ranlib", NM="llvm-nm",
                      LIBS=shlex.join(ld), CC_FOR_BUILD="cc")
    if args.build_executable:
        # Libtool must not treat crt objects as inputs to every static archive.
        wrapper = out / "target-cc"
        wrapper.write_text("#!/bin/sh\ncase \" $* \" in *' -c '*|*' -E '*|*' -S '*) exec " +
                           shlex.join(cc) + " \"$@\";; esac\nexec " + shlex.join(cc) +
                           " \"$@\" " + shlex.join(ld) + "\n")
        wrapper.chmod(0o755)
        os.environ.update(CC=str(wrapper), LIBS="")
    build_machine = run("sh", x11 / "config.guess", output=True).strip()

    def configure(source, *options, host=True):
        build = out / (source.name + "-build")
        build.mkdir()
        args = ["--prefix=" + str(prefix)]
        if host:
            args += ["--build=" + build_machine, "--host=aarch64-none-elf"]
        run("sh", source / "configure", *args, *options, cwd=build)
        run("make", "-j1", "install", cwd=build)

    # The XML and Python generator run on the build machine.
    target_env = dict(os.environ)
    for key in ("CC", "CFLAGS", "AR", "NM", "RANLIB", "LIBS"):
        os.environ.pop(key)
    configure(out / "xcb-proto-1.17.0", host=False)
    os.environ.update(target_env)
    # libxcb's host Python generator imports the xcbgen installed above.
    if args.build_executable:
        packages = list(prefix.rglob("xcbgen/__init__.py"))
        if len(packages) != 1:
            raise ValueError("expected one installed host xcbgen package")
        os.environ["PYTHONPATH"] = os.pathsep.join(filter(None, (str(packages[0].parents[1]), os.environ.get("PYTHONPATH"))))
    if args.build_executable:
        md_source = args.libmd_source.resolve()
        if hashlib.sha256(md_source.read_bytes()).hexdigest() != "1bd6aa42275313af3141c7cf2e5b964e8b1fd488025caf2f971f43b00776b332":
            raise ValueError("libmd 1.1.0 source hash mismatch")
        run("tar", "xf", md_source, "-C", out)
        configure(out / "libmd-1.1.0", "--disable-shared", "--enable-static")
        check_archive(prefix / "lib/libmd.a")
        shutil.copy(out / "libmd-1.1.0/COPYING", out / "libmd-COPYING")
    configure(out / "libpthread-stubs-0.5")
    configure(out / "libXau-1.0.11", "--disable-shared", "--enable-static", "--disable-xthreads")
    if args.build_executable:
        sdk = HERE.parents[1] / "sdk/native-aarch64/include"
        run("patch", "--fuzz=0", "-p1", "-i", HERE / "xlib-single-thread-diagnostics.patch", cwd=x11)
        for header in ("netinet/in.h", "arpa/inet.h"):
            dest = prefix / "include" / header
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(sdk / header, dest)
        run("patch", "-p1", "-i", HERE / "xcb-native-stream.patch", cwd=out / "libxcb-1.17.0")
        run("patch", "-p1", "-i", HERE / "xcb-util-local.patch", cwd=out / "libxcb-1.17.0")
    configure(out / "libxcb-1.17.0", "--disable-shared", "--enable-static")
    malloc_options = ["--enable-malloc0returnsnull"] if args.build_executable else []
    transport_options = []
    if args.build_executable:
        inc = out / "xtrans-include"
        shutil.copytree(protocols / "include/X11/Xtrans", inc / "X11/Xtrans")
        run(sys.executable, HERE / "input-codes.py",
            os.environ.get("JNOX_INPUT_EVENT_CODES", "/usr/include/linux/input-event-codes.h"), inc)
        if hashlib.sha256((inc / "X11/Xtrans/Xtrans.c").read_bytes()).hexdigest() != "bba1bbccec1ea44213cb67d2a77628ee1790c3fdbadbb755721927da52785f41":
            raise ValueError("unexpected Xtrans.c input")
        for name in ("xtrans-1.4.0-uname.patch", "xtrans-unix-only.patch"):
            run("patch", "--fuzz=0", "-p1", "-i", HERE / "identity" / name, cwd=inc)
        # Automake prepends protocol pkg-config inputs to CFLAGS.
        # Apply the private headers at compiler entry for X11 and the server.
        private_args = ["-I" + str(inc), "-I" + str(os_prefix / "include/jnox-wayland"),
                        "-DJNOX_XTRANS_LOCAL=1", "-DNO_TCP_H=1"]
        x11cc = out / "x11-cc"
        x11cc.write_text("#!/bin/sh\nexec " + shlex.join([str(wrapper), *private_args]) + ' "$@"\n')
        x11cc.chmod(0o755)
        os.environ["CC"] = str(x11cc)
        rtcfg["binaries"]["c"] = repr([*cc, *private_args])
        transport_options = ["--enable-unix-transport", "--disable-tcp-transport",
                             "--disable-local-transport"]
    configure(x11, *transport_options, *malloc_options, "--disable-shared", "--enable-static", "--disable-xthreads", "--enable-xkb",
              "--disable-loadable-i18n", "--disable-loadable-xcursor", "--disable-composecache",
              "--disable-specs", "--disable-xf86bigfont")
    for archive_name in ("libXau.a", "libxcb.a", "libX11.a"):
        check_archive(prefix / "lib" / archive_name)
    cross = out / "graphics.ini"
    rtcfg["built-in options"]["c_args"] = repr(flags + ["-D_DEFAULT_SOURCE"])
    rtcfg["built-in options"]["c_link_args"] = repr(ld + ["-L" + str(prefix / "lib")])
    rtcfg["properties"]["pkg_config_libdir"] = repr(pcdirs)
    rtcfg["binaries"]["cmake"] = "'/bin/false'"
    with cross.open("w") as f:
        rtcfg.write(f)
    meson = os.environ.get("MESON", "meson")
    run(meson, "setup", out / "xkb-build", xkb, "--cross-file", cross, "--wrap-mode=nofallback",
        "--prefix", prefix, "--libdir", "lib", "-Ddefault_library=static")
    run("ninja", "-C", out / "xkb-build", "-j1", "install")
    if check_archive(prefix / "lib/libxkbfile.a") != len(objects):
        raise ValueError("incomplete installed libxkbfile")
    for dep in ("x11", "xkbfile", "xcb", "xau"):
        location = Path(run("pkg-config", "--variable=pcfiledir", dep, output=True).strip()).resolve()
        if location != prefix / "lib/pkgconfig":
            raise ValueError("wrong target metadata provider for " + dep)
    libs = shlex.split(run("pkg-config", "--static", "--libs", "xkbfile", output=True))
    probe(out, cc, flags, libs, ld, True)
    release = args.server.resolve()
    if hashlib.sha256(release.read_bytes()).hexdigest() != "737e612ca36bbdf415a911644eb7592cf9389846847b47fa46dc705bd754d2d7":
        raise ValueError("Xwayland 24.1.6 release hash mismatch")
    run("tar", "xf", release, "-C", out)
    server = out / "xwayland-24.1.6"
    if args.build_executable:
        # The intake patch's first hunk skips mandatory X11. Keep the upstream
        # dependency in production, applying only the native socket selection.
        run("patch", "-p1", "-i", HERE / "server-unix.patch", cwd=server)
        run("patch", "-p1", "-i", HERE / "server-local-access.patch", cwd=server)
        run("patch", "--fuzz=0", "-p1", "-i", HERE / "server-broker.patch", cwd=server)
        run("patch", "--fuzz=0", "-p1", "-i", HERE / "server-busfault.patch", cwd=server)
        run("patch", "--fuzz=0", "-p1", "-i", HERE / "software-backend.patch", cwd=server)
    else:
        run("patch", "-p1", "-i", HERE / "jnox-server.patch", cwd=server)
    selected_options = []
    if args.selected_xkm:
        source_map, compiler, library = args.selected_xkm
        run(sys.executable, HERE / "selected-xkm.py", "--server", server,
            "--input-rs", source_map, "--host-deb", compiler, "--host-xkbfile-deb", library,
            "--out", out / "selected-xkm")
        selected_options = ["-Djnox_selected_xkm=true", "-Dxkb_default_rules=jnox",
                            "-Dxkb_default_model=jnox", "-Dxkb_default_layout=jnox",
                            "-Dxkb_default_variant=", "-Dxkb_default_options="]
    server_archives = []
    if args.native_xauthority:
        archive = os_prefix / "lib/libjnox-xauthority.a"
        check_archive(archive)
        run("patch", "-p1", "-i", HERE / "native-xauthority.patch", cwd=server)
        server_archives.append(str(archive))
    if args.build_executable:
        shifts = run(sys.executable, HERE / "toolchain-shifts.py", out / "shift-helpers", output=True).strip()
        if check_archive(shifts) != 2:
            raise ValueError("expected two AArch64 arithmetic helpers")
        server_archives.append(shifts)
    if server_archives:
        existing = ast.literal_eval(rtcfg["built-in options"]["c_link_args"])
        rtcfg["built-in options"]["c_link_args"] = repr([*server_archives, *existing])
        with cross.open("w") as f:
            rtcfg.write(f)
    native_options = []
    if wayland:
        host = wayland / "host"
        run(host / "bin/wayland-scanner", "--version")
        for pkg in ("xfont2", "wayland-client", "libffi", "wayland-protocols"):
            run("pkg-config", "--exists", pkg)
        os.environ["PATH"] = str(host / "bin") + os.pathsep + os.environ["PATH"]
        os.environ["PKG_CONFIG_PATH_FOR_BUILD"] = str(host / "lib/pkgconfig")
        native_options = ["--native-file", wayland / "native.ini"]
    run(meson, "setup", out / "xwayland-build", server, "--wrap-mode=nofallback",
        "--cross-file", cross, "--cross-file", HERE / "software.ini", *native_options, *selected_options,
        *(["-Dsha1=libmd", "-Ddefault_library=static"] if args.build_executable else []))
    if args.build_executable:
        build = out / "xwayland-build"
        target = "hw/xwayland/Xwayland"
        run("ninja", "-C", build, "-j1", target)
        verify_server(out)


if __name__ == "__main__":
    main()

