"""Install the native Wayland transport in a Picolibc prefix (one target at a time)."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--prefix", type=pathlib.Path, required=True)
parser.add_argument("--cc", default="clang")
parser.add_argument("--ar", default="llvm-ar")
parser.add_argument("--native-xauthority", action="store_true")
args = parser.parse_args()
here = pathlib.Path(__file__).resolve().parent
sdk = here.parent.parent
prefix = args.prefix.resolve()
if not (prefix / "lib/libc.a").is_file():
    parser.error("--prefix must contain the installed Picolibc target")
native = sdk / "include"
overlay = prefix / "include/jnox-wayland"
flags = [args.cc, "--target=aarch64-none-elf", "-std=c11", "-O2", "-ffreestanding",
         "-fno-stack-protector", "-ffunction-sections", "-fdata-sections", "-nostdlibinc"]
with tempfile.TemporaryDirectory() as temp:
    out = pathlib.Path(temp)
    def compile(name, source, options):
        subprocess.run([*flags, *options, "-c", source, "-o", out / name], check=True)
    compile("socket.o", sdk / "local_socket.c", ["-I", native, "-DJNOX_GRAPHICS_OS",
            "-Dclock_gettime=jnox_poll_clock", "-Dclock_nanosleep=jnox_poll_sleep",
            "-D__wrap_close=close", "-D__wrap_read=read", "-D__wrap_write=write",
            "-D__wrap_fcntl=jnox_transport_fcntl"])
    compile("native.o", here / "native.c", ["-I", native])
    compile("sleep.o", here / "sleep.c", ["-I", native])
    compile("public.o", here / "public.c", ["-I", here, "-isystem", prefix / "include"])
    compile("pause.o", here / "sync-pause.c", ["-isystem", prefix / "include", "-iquote", native])
    compile("identity.o", here / "identity.c", ["-isystem", prefix / "include", "-iquote", native])
    compile("sysconf.o", here / "sysconf.c", ["-isystem", prefix / "include", "-iquote", native])
    compile("credentials.o", here / "credentials.c", ["-isystem", prefix / "include", "-iquote", native])
    compile("sync.o", here / "sync.c", ["-isystem", prefix / "include", "-iquote", native, "-mno-outline-atomics"])
    subprocess.run([args.ar, "rcs", out / "libjnox-wayland-os.a", out / "socket.o",
                    out / "native.o", out / "sleep.o", out / "public.o", out / "sync.o", out / "pause.o", out / "identity.o",
                    out / "sysconf.o", out / "credentials.o"], check=True)
    for name in ("sys/socket.h", "sys/ioctl.h", "sys/un.h", "sys/uio.h", "sys/poll.h", "sys/mman.h", "poll.h"):
        target = overlay / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(native / name, target)
    shutil.copyfile(here / "sys/utsname.h", overlay / "sys/utsname.h")
    shutil.copyfile(here / "fcntl.h", overlay / "fcntl.h")
    shutil.copyfile(here / "time.h", overlay / "time.h")
    shutil.copyfile(here / "pthread.h", overlay / "pthread.h")
    shutil.copyfile(out / "libjnox-wayland-os.a", prefix / "lib/libjnox-wayland-os.a")
    if args.native_xauthority:
        headers = overlay / "jnox"
        headers.mkdir(parents=True, exist_ok=True)
        for name in ("boot_resource.h", "ipc.h"):
            shutil.copyfile(native / "jnox" / name, headers / name)
        shutil.copyfile(here / "jnox/xauthority_stream.h", headers / "xauthority_stream.h")
        compile("authority.o", here / "xauthority.c", ["-I", overlay, "-isystem", prefix / "include"])
        subprocess.run([args.ar, "rcs", out / "libjnox-xauthority.a", out / "authority.o"], check=True)
        shutil.copyfile(out / "libjnox-xauthority.a", prefix / "lib/libjnox-xauthority.a")
