# Linked fixed-priority Rust tasks on Raspberry Pi 4

[한국어](pi4-fixed-runtime.md)

CHG-033 · REQ-002/005/008/016/023 → DES-003/004/012 → IMP-031 → VER-002/005/008/016/023 → EVD-031. [#71](https://github.com/Romanof123/JNOX/issues/71)

Core zero enters at 0x80000. Other cores park before touching shared memory. EL2 startup grants nonsecure EL1 access to the physical counter and timer. A direct EL1 loader must grant this access. The existing BCM2711 GIC path arms PPI 30 with combined EOI and requires secure firmware delegation on physical hardware. The fixed runtime dispatches the same ordinary Rust functions and stack frames as virt. Pi fixed builds admit only uninstrumented FIFO and RR without Cargo features. [Board interrupt contract](../evidence/evd-014.en.md)

Run from the repository root with QEMU set to the raspi4b-capable executable. The packager verifies the pinned firmware listed in boards/pi4/firmware.json.

```sh
set -eu
out=$(mktemp -d)
for app in fixed-fifo fixed-rr; do
  (cd kernel && env -u JNOX_FIXED_TRACE JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app/target" cargo build --release --locked -j1)
  elf="$out/$app/target/aarch64-unknown-none/release/jnox-kernel"
  aarch64-linux-gnu-objcopy -O binary "$elf" "$out/$app/kernel8.img"
  python3 tools/package_pi4.py --kernel "$out/$app/kernel8.img" --output "$out/$app/boot"
  (cd "$out/$app/boot" && sha256sum -c SHA256SUMS)
  sha256sum "$elf" "$out/$app/boot/kernel8.img"
  python3 tools/qemu_boot.py --profile "pi4-$app" --kernel "$out/$app/boot/kernel8.img" --qemu "$QEMU" --runs 2 --capture-dir "$out/$app/capture"
done
```

Require exact LF-only lines, each once, with one second of silence after the final LF. FIFO order is exact. RR also permits HIGH immediately before A START under the published ordering contract.

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED FIFO START
FIXED FIFO A START
FIXED FIFO B YIELD
FIXED FIFO A RESUME
FIXED FIFO HIGH
FIXED FIFO A DONE
FIXED FIFO B SLEEP
FIXED FIFO B DONE
```

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED RR START
FIXED RR A START
FIXED RR HIGH
FIXED RR B START
FIXED RR A DONE
FIXED RR B DONE
```

Retain source revision, raw/ELF hashes, emulator version, raw UART and an exception trace from the staged image. Map IRQ and SVC entries and ERET targets against the same ELF. Identify normal Rust task return through the exit trampoline, higher-priority preemption and RR peer rotation. Measure CNTFRQ_EL0 on raspi4b and use that value for tick conversion. The 12,500,000-tick quantum is 200 ms on virt at 62.5 MHz. Measure host wall time independently. Direct model loading bypasses EEPROM and VideoCore firmware. Two physical power cycles, actual IRQ delegation, board latency and system safety approval remain open under [#34](https://github.com/Romanof123/JNOX/issues/34) and [#21](https://github.com/Romanof123/JNOX/issues/21).

Two cold boots each of FIFO and RR on the Pi 4 model showed high-priority preemption and RR peer rotation. Separate register captures measured a 62.5 MHz counter. Physical boot and timing remain open. [EVD-031](../evidence/evd-031.en.md)

CHG-062 · POL-002 · REQ/VER-002/005/008/016/023 · [Pi 4 physical UART profile plan](pi4-serial-capture.en.md#chg-062)
