# P1 physical boot bench · Raspberry Pi 4 Model B

[한국어](pi4-p1.md) · English

The proposed low-cost P1 board is the **Raspberry Pi 4 Model B, 1 GB**, with a BCM2711 and four Arm Cortex-A72 cores. The first observation is a cold boot on one core with serial output only. The project owner has no board yet. Physical results are pending. The A72 QEMU `virt` bench loads its ELF directly and uses its own memory map and UART contract. [Board issue #34](https://github.com/Romanof123/JNOX/issues/34) · [Manufacturer specifications](https://www.raspberrypi.com/products/raspberry-pi-4-model-b/specifications/) · [Existing virtual boot bench](../m1-boot.en.md)

## Build and stage the candidate

CHG-013 · REQ-008/016 · DES-004/001 · IMP-011 · VER-008/016 · EVD-011. The manifest pins firmware tag 1.20250430 and file digests. Both the dedicated link script and firmware configuration specify a raw image at `0x80000`. The boot core accepts EL2 or EL1 entry and establishes its EL1 stack. Other cores reaching this entry are parked. Low peripheral mode exposes PL011 at `0xfe201000`, with a configured 48 MHz clock and baud divisors 26 + 3/64. Timer IRQs and SMP have separate acceptance cases. [config.txt](../../boards/pi4/config.txt) · [Firmware](../../boards/pi4/firmware.json)

From the repository root, use Rust 1.95.0 with aarch64-unknown-none, GNU AArch64 binutils and Python 3.

```sh
(cd kernel && JNOX_BOARD=pi4 cargo build --release --locked -j1)
aarch64-linux-gnu-objcopy -O binary kernel/target/aarch64-unknown-none/release/jnox-kernel kernel8.img
python3 tools/package_pi4.py --kernel kernel8.img --output pi4-boot
(cd pi4-boot && sha256sum -c SHA256SUMS)
```

The packager fetches the pinned official files or takes a previously downloaded firmware boot tree through `--firmware-dir`. Copy the **contents of pi4-boot/**, including overlays, to the root of a prepared FAT32 boot partition. Run `sha256sum -c SHA256SUMS` from the card root before ejecting. Use the default firmware Arm stub. Disable EEPROM BOOT_UART and firmware uart_2ndstage diagnostics for the scored serial capture. With QEMU supporting raspi4b, run the raw payload directly as shown below. This route bypasses the EEPROM, microSD and VideoCore boot firmware. The QEMU board model has 2 GiB RAM.

```sh
python3 tools/qemu_boot.py --profile pi4 --kernel kernel8.img
```

## Candidate handoff

| Owner | What to freeze before handing the candidate to a verifier |
| --- | --- |
| Systems and BSP | Identify the board revision, 1 GB variant, EEPROM bootloader version/configuration and microSD boot order. Pin the FAT boot files and hashes, including the selected Pi 4 firmware/fixup pair, board DTB, overlays and `config.txt`. Select AArch64 and name the dedicated raw payload explicitly. Record the chosen Arm stub, actual load address, linked base, entry offset, usable RAM and reserved regions. Check the link map against the firmware settings. [Boot files](https://www.raspberrypi.com/documentation/computers/configuration.html#boot-folder-contents) · [Boot configuration and Arm stub](https://www.raspberrypi.com/documentation/computers/config_txt.html#armstub) |
| BSP and kernel | Record the stub's documented entry state and read `CurrentEL` at entry. Define the path from each supported entry level to EL1, the stack and BSS setup and the parking of secondary cores. For the first bench select polled PL011 UART0 on GPIO14 TX. The Pi 4 normally assigns UART0 to Bluetooth. Pin the routing, GPIO function, CPU-visible MMIO base, UART input clock and divisors for 115200 baud. Implement the pin and UART setup in board startup. Record the timer source, interrupt-controller mode, addresses and routing before the IRQ stage. [UART routing](https://www.raspberrypi.com/documentation/computers/configuration.html#configure-uarts) · [BCM2711 peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) |
| Integration | Supply the source revision, pinned toolchain, build and packaging commands, ELF/link map, raw payload and SHA-256 manifest. Include microSD layout, exact file names, copy steps and readback hashes for the card. Disable firmware and bootloader serial diagnostics on the capture line for the scored run. Preserve any diagnostic runs separately. |

## First physical capture

1. Use the named board, microSD card and reader, a 5 V / 3 A USB-C supply, a **3.3 V logic** USB-UART adapter and two jumper wires. With board power removed, connect adapter GND to physical header **pin 6 (GND)** first, then adapter RX to **pin 8 (GPIO14, Pi TX)**. Leave adapter TX and VCC disconnected. Power the Pi through USB-C. [Manufacturer header pinout](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html#gpio) · [UART voltage and pin assignment](https://www.raspberrypi.com/documentation/computers/configuration.html#configure-uarts)
2. Check the card contents against the candidate manifest. With the Pi off, open the adapter in raw receive mode at **115200, 8 data bits, no parity, 1 stop bit**, with flow control, echo and newline translation disabled. Drain stale input, start a new binary capture and note the USB-C power-on event on the host monotonic clock. Keep host timestamps and diagnostics in a separate file.
3. Require exactly the following 29 ASCII bytes, with one LF (`0a`) after each line, including the last. Both lines come from the same serial write after the boot core reaches EL1 and confirms that startup cleared the dirtied BSS probe.

   ```text
   JNOX PI4 BOOT
   JNOX PI4 READY
   ```

   The final LF must arrive within **30 s** of power application. Observe **1 s** of silence immediately after it with power and the receiver still on. Fail for extra bytes (including CR), wrong order, a duplicate, incomplete output or timeout. A disconnected receiver or unobserved power-on invalidates the capture. Both windows govern the host observation.
4. Remove power completely. Repeat with the same card and payload and a fresh capture. Both cold starts must pass. Store board revision, EEPROM configuration, card readback hashes, supply, adapter/port, capture tool/version, per-run power and receive times, raw bytes, stop reason and independent verdict. [Evidence template](../templates/verification-evidence.en.md)

## What each result establishes

| Stage | Evidence accepted at this stage |
| --- | --- |
| Host payload | Rebuild hashes, link map, raw image and card manifest establish the package identity and layout. |
| Pi 4 model, when available | Record the emulator version, machine, modeled and bypassed firmware/peripherals, and raw UART. A generic `virt` run stays in the virtual P1 baseline. |
| Physical cold start | The two captures establish the boot/UART contract on the recorded Pi revision. |
| Physical timer, IRQ and SMP | First identify the timer IRQ, interrupted PC, higher-priority dispatch and restoration on one core. In a later SMP case record secondary-core startup, per-core IDs, affinity and an inter-core wake-up under contention. Preserve separate verdicts. |
| Product timing and safety | Allocate board-specific timing limits, analyze interference and measure the intended product configuration under load. Functional safety and integration review the bounds, assumptions and approval scope. P2 Cortex-R52 has its own port and evidence. [Verification plan](../verification-plan.en.md) |

[EVD-011](../evidence/evd-011.en.md)
