#!/usr/bin/env python3
"""Build the opt-in JNOX graphics runtime and real Pixman probe, offline."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[3]
SDK = ROOT / "sdk/native-aarch64"
PORT = SDK / "graphics"
PROBE = ROOT / "examples/native-pixman"
DSC = "85842358d84d1f95b516f665a34132d48fc457a73cdabb3bd5bc9c6e1d198ae3"
PICO = "d1fc2a20f217472f8d9da084a519a25678e8d91852a41c7f3f1840d3508f7163"
PIX = "89a4c1e1e45e0b23dffe708202cb2eaffde0fe3727d7692b2e1739fec78a7dac"
SIGNER = "C383B778255613DFDB409D91DB221A6900000011"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args, capture=False):
    return subprocess.run([str(a) for a in args], check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def cross_file(path, compiler, cflags, ldflags, system):
    # Meson string arrays use single-quoted literals.
    def array(args):
        if any("'" in str(a) or "\\" in str(a) or "\n" in str(a) for a in args):
            raise ValueError("cross paths must fit a Meson single-quoted string")
        return "[" + ", ".join("'" + str(a) + "'" for a in args) + "]"

    path.write_text("[binaries]\nc = " + array(compiler) +
        "\nar = 'llvm-ar'\nas = 'llvm-as'\nnm = 'llvm-nm'\nstrip = 'llvm-strip'\npkgconfig = 'pkg-config'\n"
        f"[host_machine]\nsystem = '{system}'\ncpu_family = 'aarch64'\ncpu = 'cortex-a72'\nendian = 'little'\n"
        "[properties]\nneeds_exe_wrapper = true\nlibgcc = '-lclang_rt.builtins'\n"
        "[built-in options]\ndefault_library = 'static'\nb_staticpic = false\nc_args = " +
        array(cflags) + "\nc_link_args = " + array(ldflags) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--keyring", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    inputs, work = args.inputs.resolve(), args.out.resolve()
    work.mkdir(parents=True, exist_ok=False)
    dsc = inputs / "picolibc_1.8.10-2.dsc"
    pico = inputs / "picolibc_1.8.10.orig.tar.gz"
    pix = inputs / "pixman_0.44.0.orig.tar.gz"
    for archive, checksum in ((dsc, DSC), (pico, PICO), (pix, PIX)):
        if digest(archive) != checksum:
            raise ValueError("source digest mismatch: " + str(archive))
    status = run("gpgv", "--status-fd", "1", "--keyring", args.keyring.resolve(), dsc, capture=True)
    if "[GNUPG:] VALIDSIG " + SIGNER + " " not in status or PICO not in dsc.read_text():
        raise ValueError("wrong Picolibc DSC signer or archive")
    (work / "signature.txt").write_text(status)
    for archive in (pico, pix):
        run("tar", "-xf", archive, "-C", work)
    source = work / "picolibc-1.8.10"
    pixsource = work / "pixman-0.44.0"
    prefix = work / "target"
    meson = os.environ.get("MESON", "meson")
    for name in ("CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "LIBRARY_PATH",
                 "CFLAGS", "CPPFLAGS", "LDFLAGS", "PKG_CONFIG_SYSROOT_DIR", "PKG_CONFIG_PATH"):
        os.environ.pop(name, None)
    os.environ.update(LC_ALL="C", SOURCE_DATE_EPOCH="1745997199",
                      PKG_CONFIG_LIBDIR=str(prefix / "lib/pkgconfig"))
    cc = ["clang", "--target=aarch64-none-elf", "-nostdlib"]
    common = ["-mcpu=cortex-a72", "-mno-outline-atomics", "-ffreestanding",
              "-fno-stack-protector", "-fno-pie", "-fno-pic", "-fno-unwind-tables",
              "-fno-asynchronous-unwind-tables", "-ffunction-sections", "-fdata-sections"]
    cross_file(work / "pico.ini", cc, common, ["-nostdlib", "-static", "-fuse-ld=lld"], "none")
    base = ["--prefix", prefix, "--libdir", "lib", "--wrap-mode=nofallback", "--buildtype=minsize"]
    run(meson, "setup", work / "pico-build", source, "--cross-file", work / "pico.ini", *base,
        "-Dmultilib=false", "-Dtests=false", "-Dthread-local-storage=false", "-Dsingle-thread=true",
        "-Dpicocrt=false", "-Dpicocrt-lib=false", "-Dsemihost=false", "-Dspecsdir=none",
        "-Dinitfini-array=true", "-Dinitfini=false", "-Dpicoexit=true",
        "-Dformat-default=integer", "-Dassert-verbose=true", "-Dnewlib-nano-malloc=true")
    run("ninja", "-C", work / "pico-build", "-j1", "install")
    licenses = prefix / "share/licenses/jnox-graphics"
    licenses.mkdir(parents=True)
    for f in sorted(source.glob("COPYING*")):
        shutil.copy2(f, licenses / f.name)
    shutil.copy2(pixsource / "COPYING", licenses / "COPYING.pixman")
    # Keep Clang's target builtin headers (including float.h), while excluding
    # every platform libc search directory. Meson strips explicit default
    # resource paths from its compiler arguments.
    flags = [*common, "-nostdlibinc", "-isystem", str(prefix / "include"),
             "-DPIXMAN_NO_TLS", "-iquote", str(SDK / "include")]
    for obj, file in (("start.o", PORT/"start.S"), ("runtime.o", PORT/"runtime.c"),
                      ("fp.o", PROBE/"fp.S")):
        run(*cc, *flags, "-Os", "-c", file, "-o", work / obj)
    ld = ["-static", "-fuse-ld=lld", "-Wl,--gc-sections,-z,max-page-size=4096,--build-id=none",
          "-Wl,-T," + str(PORT/"app.ld"), str(work/"start.o"), str(work/"runtime.o"),
          str(prefix/"lib/libc.a"), str(prefix/"lib/libm.a")]
    cross_file(work / "jnox.ini", cc, flags, ld, "jnox")
    run(meson, "setup", work/"pix-build", pixsource, "--cross-file", work/"jnox.ini", *base,
        "-Dauto_features=disabled", "-Dtls=disabled", "-Dtests=disabled", "-Ddemos=disabled")
    run("ninja", "-C", work/"pix-build", "-j1", "install")
    config = (work/"pix-build/pixman/pixman-config.h").read_text()
    for probe in ("CONSTRUCTOR", "DESTRUCTOR"):
        if "#define TOOLCHAIN_SUPPORTS_ATTRIBUTE_" + probe + " 1" not in config:
            raise ValueError("Pixman startup feature missing: " + probe)
    run(*cc, *flags, "-I" + str(prefix/"include/pixman-1"), "-Os", "-c", PROBE/"probe.c",
        "-o", work/"probe.o")
    run(*cc, *common, work/"probe.o", work/"fp.o", "-Wl,-Map,"+str(work/"probe.map"),
        str(prefix/"lib/libpixman-1.a"), *ld, "-o", work/"probe.elf")
    report = run("readelf", "-W", "-h", "-l", "-s", work/"probe.elf", capture=True)
    (work/"probe.readelf").write_text(report)
    symbols = run("llvm-nm", work/"probe.elf", capture=True)
    for name in ("pixman_constructor", "pixman_image_composite32", "malloc", "free",
                 "sqrt", "floor", "__libc_init_array", "__libc_fini_array", "__heap_start", "__heap_end"):
        if not any(line.endswith(" "+name) for line in symbols.splitlines()):
            raise ValueError("required target symbol missing: "+name)
    if any(line.strip().startswith("U ") for line in symbols.splitlines()):
        raise ValueError("unresolved guest symbols")
    elf = (work/"probe.elf").read_bytes()
    header = struct.unpack_from("<16sHHIQQQIHHHHHH",elf)
    if header[0][:6] != b"\x7fELF\x02\x01" or header[1:3] != (2,183) or header[4] != 0x400000:
        raise ValueError("expected native static little-endian AArch64 ELF")
    loads=[]
    for i in range(header[10]):
        kind, perm, _, va, _, file_size, mem_size, _ = struct.unpack_from(
            "<IIQQQQQQ",elf,header[5] + i*header[9])
        if kind in (2,3,7):
            raise ValueError("host dynamic loader or TLS is outside this profile")
        if kind == 1:
            if not (0x400000 <= va < va+mem_size <= 0x800000 and file_size <= mem_size):
                raise ValueError("load outside native private image")
            loads.append(perm)
    if loads != [5,4,6]:
        raise ValueError("unexpected R-X / R-- / RW- layout")
    (work/"manifest.json").write_text(json.dumps({
        "picolibc_sha256": PICO, "pixman_sha256": PIX, "signer": SIGNER,
        "clang": run("clang", "--version", capture=True).splitlines()[0],
        "meson": run(meson, "--version", capture=True).strip(),
        "artifacts": {str(f.relative_to(work)): digest(f) for f in
                      (prefix/"lib/libc.a", prefix/"lib/libm.a", prefix/"lib/libpixman-1.a",
                       work/"probe.elf", work/"probe.map", work/"pico.ini", work/"jnox.ini")}
    }, indent=2)+"\n")


if __name__ == "__main__":
    main()
