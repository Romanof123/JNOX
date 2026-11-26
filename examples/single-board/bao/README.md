# Bao two-domain board

Build under Linux or WSL with Git, Python 3, GNU make, GCC for the build host, and GNU AArch64 cross tools. Run from the project root with a fresh build directory on a Linux filesystem.

```sh
bash examples/single-board/bao/build.sh /mnt/d/codex/jnox422-pm/bao-v2 \
    "$linux_out/linux.bin" "$native_out/jnox.elf" "$bao_out"
qemu-system-aarch64 -machine virt,virtualization=on,secure=off,gic-version=2 \
    -cpu cortex-a72 -smp 2 -m 512M -accel tcg -nodefaults -display none \
    -monitor none -serial stdio -kernel "$bao_out/bao.bin" \
    -global virtio-mmio.force-legacy=false -netdev user,id=lan,restrict=on \
    -device virtio-net-device,netdev=lan,mac=52:54:00:12:34:56
```

The builder checks out Bao v2.0.0 commit `0af4a1ab558ad60c9658af4f43756c4536dd3141` in the output directory. It adjusts the upstream platform description and checks the final link offsets, embedded bytes, source PA, destination PA, and upstream IPA overlap test for each VM. Bao embeds both raw guest images and copies them into their assigned RAM. The Linux image begins with the handoff stub produced by the sibling Linux builder.

| Domain | Physical CPU | Guest RAM IPA | Physical RAM backing | Guest entry |
| --- | --- | --- | --- | --- |
| Linux | 0 | 0x50000000–0x5bffffff | 0x50000000–0x5bffffff | 0x50000000 |
| JNOX | 1 | 0x40000000–0x47ffffff | 0x48000000–0x4fffffff | 0x40200000 |

Both virtual GICv2 interfaces use distributor 0x08000000 and CPU interface 0x08010000. PPI 27 is private to each assigned CPU. Bao prints boot diagnostics on PL011. Linux alone receives the guest PL011 mapping at 0x09000000 and IRQ 33. Its network transport at 0x0a003e00 uses IRQ 79. Stage-2 maps the containing 4 KiB page, which also contains seven unpopulated virtio slots. JNOX records its witness in RAM. For a QEMU physical read, add 0x08000000 to the JNOX journal IPA.

The QEMU arm64 direct kernel loader starts the headerless Bao binary at RAM base plus 0x80000. Bao enters at EL2 and uses PSCI CPU_ON via SMC to start CPU 1 at the same physical entry. QEMU virt provides the SMC conduit with virtualization enabled and firmware-free boot. [QEMU virt PSCI selection](https://github.com/qemu/qemu/blob/v9.2.0/hw/arm/virt.c) and [arm64 raw image loading](https://github.com/qemu/qemu/blob/v9.2.0/hw/arm/boot.c).
