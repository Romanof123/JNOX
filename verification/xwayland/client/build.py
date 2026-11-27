#!/usr/bin/env python3
"""Prepare real host protocol tools and cross-build libffi and libwayland-client."""
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

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("phase", choices=["prepare", "protocols", "ffi", "client"])
parser.add_argument("inputs", type=Path)
parser.add_argument("runtime", type=Path, help="frozen graphics runtime containing jnox.ini and target/")
parser.add_argument("work", type=Path, help="new for prepare; reused by later phases")
args = parser.parse_args()
inputs, runtime, work = (p.resolve() for p in (args.inputs, args.runtime, args.work))
host, target = work/"host", work/"target"
source = work/"wayland-1.23.1"
meson = os.environ.get("MESON", "meson")

def run(*cmd, env=None, cwd=None):
    print("+", shlex.join(list(map(str,cmd))), flush=True)
    subprocess.run(list(map(str,cmd)), check=True, env=env, cwd=cwd)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def pc_env(path):
    env = os.environ.copy()
    for key in ("CFLAGS", "CPPFLAGS", "LDFLAGS", "CPATH", "C_INCLUDE_PATH", "LIBRARY_PATH", "PKG_CONFIG_SYSROOT_DIR"):
        env.pop(key, None)
    env.update(PKG_CONFIG_PATH="", PKG_CONFIG_LIBDIR=str(path))
    return env

def read_cross():
    cross = configparser.ConfigParser(interpolation=None)
    cross.read(runtime/"jnox.ini")
    options = cross["built-in options"]
    return (ast.literal_eval(cross["binaries"]["c"]), ast.literal_eval(options["c_args"]),
            ast.literal_eval(options["c_link_args"]))

def archive_check(path, symbols):
    headers = subprocess.check_output(["readelf", "-hW", path], text=True)
    machines = re.findall(r"^\s*Machine:\s*(.+)$", headers, re.M)
    if not machines or set(machines) != {"AArch64"}:
        raise ValueError(f"{path}: unexpected ELF machines {machines}")
    names = subprocess.check_output(["llvm-nm", "-g", "--defined-only", path], text=True)
    for sym in symbols:
        if not re.search(r"\b[TW] "+re.escape(sym)+r"$", names, re.M):
            raise ValueError(f"{path}: missing definition {sym}")

if args.phase == "prepare":
    work.mkdir()
    pins = dict(line.split()[::-1] for line in (HERE/"inputs.sha256").read_text().splitlines())
    for file, digest in pins.items():
        if sha(inputs/file) != digest:
            raise ValueError("input mismatch: " + file)
    for archive, descriptor in [
        ("wayland_1.23.1.orig.tar.gz","wayland_1.23.1-3.dsc"),
        ("wayland-protocols_1.44.orig.tar.xz","wayland-protocols_1.44-1.dsc"),
        ("libffi_3.4.4.orig.tar.gz","libffi_3.4.4-1.dsc")]:
        entry = f"{pins[archive]} {(inputs/archive).stat().st_size} {archive}"
        if entry not in (inputs/descriptor).read_text().split("Checksums-Sha256:\n",1)[1].split("\nFiles:",1)[0]:
            raise ValueError("descriptor mismatch: " + archive)
        run("tar", "xf", inputs/archive, "-C", work)
    run("dpkg-deb", "-x", inputs/"libexpat1-dev_2.4.7-1ubuntu0.10_amd64.deb", work/"expat")
    metadata = work/"expat/usr/lib/x86_64-linux-gnu/pkgconfig/expat.pc"
    metadata.write_text(metadata.read_text().replace("prefix=/usr\n", f"prefix={work}/expat/usr\n"))
    shutil.copytree(source, work/"client-source")
    run("patch", "-p1", "-i", HERE/"client-only.patch", cwd=work/"client-source")
    run("patch", "-p1", "-i", HERE/"native-diagnostics.patch", cwd=work/"client-source")
    manifest = json.loads((runtime/"manifest.json").read_bytes())
    for name in ("target/lib/libc.a", "target/lib/libm.a", "target/lib/libpixman-1.a"):
        if sha(runtime/name) != manifest["artifacts"][name]:
            raise ValueError("graphics runtime artifact mismatch: "+name)
    cc, flags, ld = read_cross()
    if "--target=aarch64-none-elf" not in cc or "-nostdlib" not in cc or "-nostdlibinc" not in flags or str(runtime/"target/include") not in flags:
        raise ValueError("unexpected target compiler or Picolibc include search")
    (work/"native.ini").write_text("[built-in options]\npkg_config_path = ['"+str(host/"lib/pkgconfig")+"']\n")
    (work/"target.ini").write_text("[binaries]\ncmake = '/bin/false'\n[properties]\npkg_config_libdir = ['"+str(target/"lib/pkgconfig")+"', '"+str(target/"share/pkgconfig")+"']\n")
elif args.phase == "protocols":
    env = pc_env(work/"expat/usr/lib/x86_64-linux-gnu/pkgconfig")
    run(meson, "setup", work/"scanner-build", source, "--prefix", host, "--libdir", "lib",
        "--wrap-mode=nofallback", "-Dlibraries=false", "-Dscanner=true", "-Dtests=false",
        "-Ddocumentation=false", "-Ddtd_validation=false", env=env)
    run("ninja", "-j1", "-C", work/"scanner-build", "install", env=env)
    scanner = host/"bin/wayland-scanner"
    run(scanner, "--version")
    env = pc_env(host/"lib/pkgconfig")
    run(meson, "setup", work/"protocol-build", work/"wayland-protocols-1.44", "--prefix", target,
        "--wrap-mode=nofallback", "-Dtests=false", env=env)
    run("ninja", "-j1", "-C", work/"protocol-build", "install", env=env)
    for relative in ("stable/xdg-shell/xdg-shell.xml", "staging/xwayland-shell/xwayland-shell-v1.xml",
                     "staging/linux-drm-syncobj/linux-drm-syncobj-v1.xml"):
        xml = target/"share/wayland-protocols"/relative
        for mode in ("client-header","private-code"):
            run(scanner, "--strict", mode, xml, work/(xml.stem+"."+mode))
elif args.phase == "ffi":
    cc, flags, ld = read_cross()
    env = pc_env(target/"lib/pkgconfig")
    env.update(CC=shlex.join(cc), CFLAGS=shlex.join(flags), LDFLAGS=shlex.join(ld),
               AR="llvm-ar", RANLIB="llvm-ranlib", NM="llvm-nm")
    build = work/"ffi-build"
    build.mkdir()
    run(work/"libffi-3.4.4/configure", "--host=aarch64-none-elf", "--build=x86_64-pc-linux-gnu",
        "--prefix="+str(target), "--libdir="+str(target/"lib"), "--disable-shared", "--enable-static",
        "--disable-multi-os-directory", "--disable-docs", env=env, cwd=build)
    # Configure links executable probes with the native startup objects.
    # Libtool must never insert those application objects into libffi.a.
    makefile = build / "Makefile"
    prepared, count = re.subn(r"^LDFLAGS = .*$", "LDFLAGS =", makefile.read_text(), flags=re.M)
    if count != 1:
        raise ValueError("unexpected generated libffi archive link settings")
    makefile.write_text(prepared)
    run("make", "-j1", "install", env=env, cwd=build)
    archive_check(target/"lib/libffi.a", ["ffi_call","ffi_prep_cif"])
else:
    archive_check(target/"lib/libffi.a", ["ffi_call", "ffi_prep_cif"])
    env = pc_env(target/"lib/pkgconfig")
    env["PKG_CONFIG_PATH_FOR_BUILD"] = str(host/"lib/pkgconfig")
    cc, flags, ld = read_cross()
    port = work/"client-options.ini"
    port.write_text("[built-in options]\nc_args = " + repr(["-D_GNU_SOURCE", *flags]) + "\n")
    run(meson, "setup", work/"client-build", work/"client-source", "--cross-file", runtime/"jnox.ini",
        "--cross-file", work/"target.ini", "--cross-file", port, "--native-file", work/"native.ini", "--prefix", target,
        "--libdir", "lib", "--wrap-mode=nofallback", "-Ddefault_library=static", "-Dscanner=false",
        "-Dtests=false", "-Ddocumentation=false", env=env)
    run("ninja", "-j1", "-C", work/"client-build", "install", env=env)
    archive = target/"lib/libwayland-client.a"
    archive_check(archive, ["wl_display_connect_to_fd", "wl_display_roundtrip", "wl_proxy_marshal_flags",
                            "wl_connection_flush", "wl_list_init", "wl_os_recvmsg_cloexec"])
    cc, flags, ld = read_cross()
    run(*cc, *flags, "-I"+str(target/"include"), "-c", HERE/"roundtrip.c", "-o", work/"roundtrip.o", env=env)
    run(*cc, work/"roundtrip.o", archive, target/"lib/libffi.a", *ld,
        "-Wl,-Map,"+str(work/"roundtrip.map"), "-o", work/"roundtrip.elf", env=env)
    run("readelf", "-hW", "-lW", work/"roundtrip.elf")
