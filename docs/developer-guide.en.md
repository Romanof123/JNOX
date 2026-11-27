# J-NOX developer guide

English · [한국어](developer-guide.ko.md)

Use this walkthrough to package a C program and a Rust program into the same AArch64 kernel, observe their separate exits on UART, and select a driver integration path. Commands target the public J-NOX revision `5dabcd904a83b0301650c971d4eba4e884fd1eb5` on Linux or WSL. The example runs one Cortex-A72 under QEMU TCG, with GICv2, 128 MiB and the native FIFO scheduler. [Source snapshot](https://github.com/Romanof123/JNOX/tree/5dabcd904a83b0301650c971d4eba4e884fd1eb5)

## Prepare the build host

Use Bash, Git, Python 3.10 or later, rustup, Clang with AArch64 support, LLD, GNU binutils including AArch64 cross-binutils, GNU coreutils and `qemu-system-aarch64`. The RTC walkthrough also uses curl and the Python package pyelftools. Keep the same shell and working directory throughout. Package directories must be new. [Kernel toolchain](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/rust-toolchain.toml) · [Rust SDK build](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/sdk/native-aarch64/build-rust.sh)

```sh
git clone https://github.com/Romanof123/JNOX.git jnox-guide
cd jnox-guide
git fetch origin refs/pull/458/head
git checkout --detach 5dabcd904a83b0301650c971d4eba4e884fd1eb5
rustup toolchain install 1.95.0 --profile minimal
export RUSTUP_TOOLCHAIN=1.95.0
rustup target add aarch64-unknown-none aarch64-unknown-none-softfloat
work=$(mktemp -d)
printf '%s\n' "$work"
```

## Package two applications

The SDK supplies startup assembly, the linker script, C headers and native Rust bindings. A C application returns an integer from `main`. The Rust example uses `#![no_std]`, `#![no_main]`, an exported C-ABI `main`, and a panic handler. Follow its Cargo dependency on the in-tree `jnox-native` crate when starting a new application. Both programs yield after every four sensor samples. The packager assigns independent process images, argument vectors and priorities. [C source](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv/sensor.c) · [Rust source](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv-rust/src/main.rs) · [Cargo manifest](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv-rust/Cargo.toml)

```sh
set -eu
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c "$work/sensor-c.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-argv-rust/Cargo.toml "$work/sensor-rs.elf"
python3 tools/build_native_el0.py --board virt --kernel-profile release \
  --policy fifo --priority 1 --peer-priority 7 --out "$work/pair" \
  --elf "$work/sensor-c.elf" --arg sum --arg 16 --arg 32 --arg west \
  --peer-elf "$work/sensor-rs.elf" --peer-arg sum --peer-arg 240 --peer-arg 256 --peer-arg east
```

Slot 0 sums samples 16 through 31 to obtain 376. Slot 1 sums 240 through 255 to obtain 3960. The example returns 17 and 43 respectively after checking its own calculation. These are application exit codes. The package manifest identifies the input ELF for each slot, its digest, arguments, priority, kernel build command and kernel digest. [Argument ABI and expected values](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv/README.md)

## Boot and check the result

```sh
status=0
timeout 20s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
  -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
  -serial "file:$work/uart.log" -kernel "$work/pair/kernel.elf" || status=$?
test "$status" -eq 0 || test "$status" -eq 124
tr -d '\r' < "$work/uart.log" > "$work/uart.txt"
grep -Fx 'NATIVE EL0 EXIT PID 0 17' "$work/uart.txt"
grep -Fx 'NATIVE EL0 EXIT PID 1 43' "$work/uart.txt"
```

The kernel parks after reporting both exits. The host timeout bounds the smoke run. Both exact UART lines must be present. Keep the original UART bytes and manifest. For a stronger check, read `sensor_sum` from each process's active mapping and compare with the expected values above. A host symbol address alone cannot distinguish the two address spaces. [Native capture reader](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/tools/native_el0_capture.py)

## Develop a native Rust driver

Start with an explicit hardware contract: device identity, owner slot, register ranges, IRQ, DMA limits, reset acknowledgement and maximum work per activation. Identify which work runs in the EL0 service and which operations require EL1. In the pinned virtio-net profile, Rust code in EL1 operates MMIO and fixed DMA queues. The Rust IPv4 service in EL0 handles endpoints and packets through the SDK's checked frame operations. Use these source boundaries when defining the grant and the unauthorized-caller test. [Native Rust transport](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/src/native_virtio_net.rs) · [EL0 IPv4 service](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/services/inet/README.md) · [DES-009](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/architecture.en.md)

For a smaller first device change, follow the PL011 IRQ example. Its Rust EL0 app reads bytes and IRQ statistics through the display-owner API. Package the owner with `--display-service 0 --uart-rx-irq`, and the second slot with argument `denied`. The documented observer sends eight bytes, overfills a 16-byte ring with 24 more, and reads the guest's report from a framebuffer scanline. It also checks the polling and headless packages. This exercises IRQ delivery, overflow and access denial on the declared machine. [Runnable three-package recipe](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-uart-rx-irq/README.md) · [EL1 interrupt handler](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/src/native_uart_rx.rs)

## Import an unchanged Linux C driver

For PL031, use the original Linux v6.18 translation unit from commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`. The importer checks the size, SHA-256, Git blob and SPDX declaration, then builds the C object against the supported Linux API adapter. It checks imported symbols and links a static J-NOX service. Keep the upstream file unchanged and implement additional supported facilities in the adapter. [Original PL031 file](https://github.com/torvalds/linux/blob/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/rtc/rtc-pl031.c) · [Intake profile](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/drivers/linux-compat/profiles/rtc-pl031.json)

```sh
curl -fL https://raw.githubusercontent.com/torvalds/linux/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/rtc/rtc-pl031.c \
  -o "$work/rtc-pl031.c"
python3 tools/import_linux_driver.py --profile rtc-pl031 --source "$work/rtc-pl031.c" --out "$work/import"
sh sdk/native-aarch64/build.sh examples/linux-source-rtc/peer.c "$work/rtc-peer.elf"
python3 tools/build_native_el0.py --board virt --linux-source-rtc --policy fifo \
  --priority 8 --peer-priority 8 --elf "$work/import/service.elf" \
  --peer-elf "$work/rtc-peer.elf" --out "$work/rtc"
python3 tools/verify_linux_source_rtc.py --package "$work/rtc" \
  --driver-manifest "$work/import/import.json" --out "$work/rtc-run"
python3 tools/verify_linux_source_rtc.py --replay --require-gic --package "$work/rtc" \
  --driver-manifest "$work/import/import.json" --out "$work/rtc-run"
```

The replay checks source and ELF identity, RTC register activity, GIC acknowledgement, peer access denial and cleanup. For a network driver, use the separate virtio-net builder and matching AArch64 headers. Its pinned input is Linux stable v6.18.65 commit `4700e3adf3b24b39dd4929e9c6d62811ba3ccea8`, file `drivers/net/virtio_net.c`. Choose the profile that exercises the operation being changed, such as RX IRQ or service restart. Archive the header package and configuration hashes with the object and service ELF. [RTC evidence and commands](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/evidence/native-linux-source-rtc/README.md) · [Original virtio-net source](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c) · [Network intake profile](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/drivers/linux-compat/profiles/virtio-net.json) · [Network builder](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/tools/build_linux_source_virtio_net.py)

## Take a change through release

1. **Define the requirement.** Identify the REQ and CHG records, board, roles, externally visible outcome and failure conditions. State timing bounds and the clock used before collecting measurements.
2. **Describe the design.** Link the DES record and identify privilege boundaries, message or register ABI, queue limits, ownership and restart policy. Specify how an old handle, pending IRQ or DMA buffer becomes unusable.
3. **Implement and package.** Link the IMP record and commit. Retain original sources and licenses, compiler versions, commands, package manifest and artifact hashes. Build a baseline and the changed profile in separate directories.
4. **Verify independently.** Link the VER case to a concrete observation: an external packet, register transition, screen capture or deadline series. Include negative controls such as an unauthorized peer or corrupted replay input. Have an independent reader verify raw records and matching ELF identities. State which claims each run covers.
5. **Release the evidence.** Link REQ, DES, IMP, VER and EVD records in the trace table. Package raw captures, hashes, replay instructions and known limits. Verify a fresh extraction and record its result. Keep QEMU observations scoped to the declared virtual machine. [Requirements](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/requirements.en.md) · [Trace table](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/trace/links.csv) · [PL031 release example](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/evidence/native-linux-source-rtc/README.md)

PM links the change issue and PR in both directions. Systems starts at the earliest affected requirement when behavior changes. Functional Safety owns the rationale for safety limits. Development teams may work concurrently. Integration, guest builds and formal verification share one execution slot. Give the first verifier the contract, candidate identity and raw inputs before sharing prior verdicts. Release records the configuration and its remaining limits. Update both editions of this guide when an SDK call, build command or expected result changes. [PR template](../.github/pull_request_template.md)

For filesystem work, begin with the packaged-file example and the staged acceptance criteria for a read-only root, volatile `/tmp`, persistent media and recovery. [CAP-007](filesystem-plan.en.md) · [#459](https://github.com/Romanof123/JNOX/issues/459)
