# Multilevel queue workloads

Build all four independent ELFs once, from the repository root in Linux or WSL. Use the same files for all variants. The CRC and Rust histogram use the existing [CBS inputs and host oracle](../native-cbs/README.md).

~~~sh
sh sdk/native-aarch64/build.sh examples/native-multilevel/report.c report.elf
sh sdk/native-aarch64/build.sh examples/native-multilevel/yield.c yield.elf
sh sdk/native-aarch64/build.sh examples/native-cbs/crc.c crc.elf
sh sdk/native-aarch64/build-rust.sh examples/native-cbs-rust/Cargo.toml histogram.elf

mlq() { python3 tools/build_native_el0.py --policy mlq --quantum-ticks 625000 "$@"; }
mlfq() { python3 tools/build_native_el0.py --policy mlfq --allotment-ticks 625000,1250000,2500000 --boost-ticks 6250000 "$@"; }

mlq --out strict --elf report.elf --class 7 --peer-elf crc.elf --peer-class 1
mlfq --out feedback --elf report.elf --peer-elf crc.elf
mlq --out strict-three --elf crc.elf --class 7 --peer-elf histogram.elf --peer-class 7 --peer-elf report.elf --peer-class 1
mlq --out equal-yield --elf yield.elf --class 7 --peer-elf crc.elf --peer-class 7
mlfq --out cumulative-yield --elf yield.elf --peer-elf crc.elf
mlfq --out solo-yield --elf yield.elf
mlfq --out idle-report --elf report.elf
~~~

Use the reference physical counter frequency of 62,500,000 Hz. Freeze the toolchain, inputs, binaries and disassemblies before the first boot. For example, launch strict on the stock virtual machine as follows:

~~~sh
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel strict/kernel.elf
~~~

Capture strict plain then externally exception-traced, followed by feedback plain then traced. An external QEMU exception log can be enabled with `-d int -D exceptions.log`; use a fresh file for each capture. Apply the [design's decision and observation criteria](../../docs/native-multilevel-applications.en-ko.md) to each journal.

Both C alarm workers regenerate 4096 volatile bytes per block using (17*i + 3 + block) modulo 256. Every block contains sixteen complete byte permutations and therefore 16×16 = 256 bytes at least 240. Map count_alarms to the packaged ELF disassembly.

| ELF | Final u32 private_blocks | Final u64 private_bytes | Final u32 private_result | Exit |
| --- | ---: | ---: | ---: | ---: |
| report | 3 | 12288 | 768 | 53 |
| yield | 16384 | 67108864 | 4194304 | 59 |

The reporter stores each absolute deadline in request_ns[3] and its first post-wake clock sample in observed_ns[3], both u64 nanoseconds. Each sample must reach its target. The u32 arrays round_count and cumulative_count must equal [256,256,256] and [256,512,768]. The yield client calls sched_yield after publishing every complete block, including the last. Use recorded service to evaluate cross-yield demotion. If the bounded journal fills before exit, retain its drop count and obtain final products independently.
