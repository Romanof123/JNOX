# Single-core timer preemption on the Pi 4

[한국어](pi4-timer.md)

CHG-016 · REQ-002/005/008/023 → DES-003/004/012 → IMP-014 → VER-002/005/008/023 → EVD-014. [#41](https://github.com/Romanof123/JNOX/issues/41) · [#34](https://github.com/Romanof123/JNOX/issues/34)

LOW and HIGH own separate stacks on core zero. The nonsecure physical timer interrupts the running LOW through GIC-400 PPI 30. The IRQ makes HIGH ready and the fixed-priority scheduler selects its frame. After HIGH completes with SVC #1, LOW resumes at its saved instruction and checks restored state.

Build from the repository root and use a new output directory.

```sh
(cd kernel && JNOX_BOARD=pi4 cargo build --release --locked --features sched-bench -j1)
aarch64-linux-gnu-objcopy -O binary kernel/target/aarch64-unknown-none/release/jnox-kernel kernel8-sched.img
python3 tools/qemu_boot.py --profile pi4-scheduler --kernel kernel8-sched.img
python3 tools/package_pi4.py --kernel kernel8-sched.img --output pi4-scheduler-boot
(cd pi4-scheduler-boot && sha256sum -c SHA256SUMS)
```

Use `--qemu` to select the emulator. For the first verification use a wrapper adding `-d int -D "$TRACE"` so the original guest supplies exception evidence.

A fresh verifier first identifies the raw payload, frozen source and ELF. Launch one raspi4b guest, require exactly 103 LF-only UART bytes and one second of silence after the final LF, then record the first verdict. Map that same guest’s `-d int` IRQ and SVC #1 against the ELF, including the interrupted LOW PC, HIGH entry and exact LOW return. Identify which interrupt-controller and state checks come from executed code and which exception addresses were observed externally.

```text
JNOX PI4 BOOT
JNOX PI4 READY
SCHED LOW RUN
SCHED TIMER IRQ
SCHED HIGH RUN
SCHED LOW RESUMED
SCHED PASS
```

Physical operation requires the boot firmware to enable the GIC path and delegate the timer to the nonsecure group, and the boot core to have EL1 permission to access the physical timer and counter. In low peripheral mode GICD is at 0xff841000 and GICC at 0xff842000. Record firmware, EEPROM, board revision and raw/ELF hashes. Follow the Pi 4 serial wiring guide. Capture two fully power-cycled starts from the same SD card. For each, receive the exact bytes below within 30 s of power-on and observe 1 s of silence. The board model bypasses the EEPROM and VideoCore firmware. UART establishes marker order. A physical interrupted-PC claim needs an on-board trace or debugger evidence, and timing approval needs separate measurements. [Pi 4 UART](pi4-p1.en.md)

[BCM2711 Arm stub](https://github.com/raspberrypi/tools/blob/master/armstubs/armstub8.S) · [BCM2711 timer / GIC](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm/boot/dts/broadcom/bcm2711.dtsi) · [QEMU BCM2838 model](https://github.com/qemu/qemu/blob/master/hw/arm/bcm2838.c)
