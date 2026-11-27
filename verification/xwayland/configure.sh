#!/bin/sh
# Usage: MESON=/path/to/meson sh configure.sh SOURCE NEW_BUILD TARGET_PREFIX
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sdk=$(CDPATH= cd -- "$here/../../sdk/native-aarch64" && pwd)
src=$(realpath "$1")
build=$(realpath -m "$2")
prefix=$(realpath -m "$3")
mkdir "$build" # preserve earlier attempts and logs
mkdir -p "$prefix/lib/pkgconfig" "$prefix/share/pkgconfig"
export PKG_CONFIG_PATH=
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
# Prevent inherited native compiler include/library paths contaminating the target.
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CFLAGS CPPFLAGS LDFLAGS
ccflags='--target=aarch64-none-elf -ffreestanding -nostdinc -fno-pic -fno-pie -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables -mgeneral-regs-only -mno-outline-atomics'
clang $ccflags -I"$sdk/include" -c "$sdk/start.S" -o "$build/start.o"
clang $ccflags -I"$sdk/include" -std=c11 -c "$here/sdk-probe.c" -o "$build/sdk-probe.o"
clang --target=aarch64-none-elf -nostdlib -static -fuse-ld=lld -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none "$build/start.o" "$build/sdk-probe.o" -o "$build/sdk-probe.elf"
readelf -hW -lW "$build/sdk-probe.elf" > "$build/sdk-probe.readelf"
cat > "$build/jnox.ini" <<EOF
[binaries]
c = ['clang', '--target=aarch64-none-elf']
ar = 'llvm-ar'
strip = 'llvm-strip'
pkgconfig = 'pkg-config'
cmake = '/bin/false'
[host_machine]
system = 'jnox'
cpu_family = 'aarch64'
cpu = 'aarch64'
endian = 'little'
[properties]
needs_exe_wrapper = true
pkg_config_libdir = ['$prefix/lib/pkgconfig', '$prefix/share/pkgconfig']
[built-in options]
default_library = 'static'
c_args = ['-ffreestanding', '-nostdinc', '-fno-pic', '-fno-pie', '-fno-stack-protector', '-fno-unwind-tables', '-fno-asynchronous-unwind-tables', '-mgeneral-regs-only', '-mno-outline-atomics', '-I$sdk/include', '-I$prefix/include']
c_link_args = ['-nostdlib', '-static', '-fuse-ld=lld', '-Wl,-T,$sdk/app.ld,-z,max-page-size=4096,--build-id=none', '$build/start.o', '-L$prefix/lib']
EOF
exec "${MESON:-meson}" setup "$build/meson" "$src" --wrap-mode=nofallback --cross-file "$build/jnox.ini" --cross-file "$here/software.ini"
