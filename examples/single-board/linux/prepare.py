#!/usr/bin/env python3
"""Package the pinned Alpine Linux driver domain for Bao's raw EL1 entry."""
import argparse
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).absolute().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE.parents[2] / "tools"))
import verify_linux_virtio_net_baseline as baseline

BASE = 0x50000000
RAM_SIZE = 0x0c000000
ALIGN = 0x200000
DTB = 0x1000


def gateway_profile(args, out, initial_irq):
    source = HERE / "gateway.c"
    program = out / "gateway"
    flags = ["-std=c11", "-Os", "-static", "-s", "-Wall", "-Wextra", "-Werror", "-Wl,--build-id=none"]
    subprocess.run([args.cc, *flags, str(source), "-o", str(program)], check=True)
    elf = program.read_bytes()
    baseline.require(elf.startswith(b"\x7fELF\x02\x01") and elf[18:20] == b"\xb7\x00",
                     "expected a static AArch64 gateway ELF")
    init = baseline.INIT_PREFIX + initial_irq + """if [ ! -c /dev/mem ]; then
    $bb mknod -m 600 /dev/mem c 1 1
fi
exec /gateway
"""
    operands = baseline.prepare(args.intake, out, init_script=init, service_files={"gateway": elf})
    operands["gateway"] = {
        "source_sha256": baseline.sha(source.read_bytes()), "elf_sha256": baseline.sha(elf),
        "compiler": subprocess.check_output([args.cc, "--version"], text=True).splitlines()[0],
        "flags": flags,
    }
    return operands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--intake", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--dtc", default="dtc")
    parser.add_argument("--cc", default="aarch64-linux-gnu-gcc")
    profile = parser.add_mutually_exclusive_group()
    profile.add_argument("--gateway", action="store_true", help="use the Bao shared-page JG service")
    profile.add_argument("--sample-network", action="store_true",
                        help="sample passive Linux IRQ and NIC counters on UART during diagnosis")
    args = parser.parse_args()
    out = args.out.absolute()
    out.mkdir(exist_ok=True, parents=True)
    irq_rows = """$bb grep -E 'virtio|eth0|uart|pl011|a003e00|arch_timer' /proc/interrupts || :"""
    initial_irq = 'echo "LD IRQ BEGIN ready"\n' + irq_rows + '\necho "LD IRQ END ready"\n'
    if args.gateway:
        operands = gateway_profile(args, out, initial_irq)
    else:
        init = baseline.INIT.replace('echo "LD READY"', initial_irq + 'echo "LD READY"', 1)
        if args.sample_network:
            sampler = (HERE / "sample-network.sh").read_text()
            init = init.replace('echo "LD READY"', sampler + '\necho "LD READY"')
        # Record the kernel IRQ counters at each of the existing HTTP requests.
        # Reading procfs leaves the device and its read-to-clear registers untouched.
        serve = baseline.SERVE.replace('case "$target" in', r'''{
    printf 'LD IRQ BEGIN %s\n' "$target"
    @IRQ_ROWS@
    printf 'LD IRQ END %s\n' "$target"
} >/dev/console
case "$target" in'''.replace("@IRQ_ROWS@", irq_rows), 1)
        operands = baseline.prepare(args.intake, out, init_script=init, service_files={"serve": serve.encode()})
        (out / "serve.sh").write_text(serve)
    kernel = (out / "Image").read_bytes()
    text_offset, image_size = struct.unpack_from("<QQ", kernel, 8)
    kernel_offset = ALIGN + text_offset
    if image_size == 0 or kernel_offset % 4 or kernel_offset >= 1 << 27:
        raise ValueError("unsupported arm64 Image header")
    initrd_offset = (kernel_offset + max(image_size, len(kernel)) + ALIGN - 1) & -ALIGN
    initrd = (out / "initrd.gz").read_bytes()
    end = initrd_offset + len(initrd)
    if end > RAM_SIZE:
        raise ValueError("Image and initramfs exceed the Linux partition")
    source = (HERE / "guest.dts.in").read_text()
    if args.gateway:
        source = source.replace("    chosen {", """    jnox-gateway@b000000 {
        compatible = "jnox,gateway-v1";
        reg = <0 0x0b000000 0 0x1000>;
    };

    chosen {""", 1)
    source = source.replace("@INITRD_START@", hex(BASE + initrd_offset))
    source = source.replace("@INITRD_END@", hex(BASE + end))
    (out / "guest.dts").write_text(source)
    subprocess.run([args.dtc, "-I", "dts", "-O", "dtb", "-o", str(out / "guest.dtb"),
                    str(out / "guest.dts")], check=True)
    dtb = (out / "guest.dtb").read_bytes()
    if DTB + len(dtb) > ALIGN:
        raise ValueError("DTB overlaps the kernel placement region")

    # adr x0, +4096; mov x1, xzr; mov x2, xzr; mov x3, xzr; b Image
    # ADR starts at BASE and the final branch PC is BASE + 16.
    stub = struct.pack("<5I", 0x10008000, 0xaa1f03e1, 0xaa1f03e2, 0xaa1f03e3,
                       0x14000000 | ((kernel_offset - 16) // 4))
    payload = bytearray(end)
    for offset, blob in ((0, stub), (DTB, dtb), (kernel_offset, kernel), (initrd_offset, initrd)):
        payload[offset:offset + len(blob)] = blob
    (out / "linux.bin").write_bytes(payload)
    baseline.save(out / "linux.json", {
        "operands": operands, "base": BASE, "ram_size": RAM_SIZE, "image_offset": kernel_offset,
        "image_memory_size": image_size, "initrd_offset": initrd_offset, "dtb_offset": DTB,
        "dtb_sha256": baseline.sha(dtb), "payload_sha256": baseline.sha(payload),
    })


if __name__ == "__main__":
    main()
