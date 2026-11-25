# Native hard-CBS sensor streams

From the repository root, use the public SDK to build standalone ELFs. Supply a fresh package output directory for each configuration.

~~~sh
for app in crc energy short_report block_yield; do
    sh sdk/native-aarch64/build.sh examples/native-cbs/"$app".c "$app".elf
done
sh sdk/native-aarch64/build-rust.sh examples/native-cbs-rust/Cargo.toml histogram.elf
python3 tools/build_native_el0.py --out isolated --policy cbs --elf crc.elf --reservation 1,4 \
    --peer-elf histogram.elf --peer-reservation 1,4
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel isolated/kernel.elf
~~~

Each stream reads the ordinary monotonic clock at entry and after each complete block. It finishes at the first block with elapsed time at least 650 ms. The cap is 1048576 blocks, with distinct exhaustion statuses 103 (C) and 116 (Rust). All applications publish u64 private_bytes and private_blocks in their own address spaces. A complete block contains 16 copies of bytes 0 through 255. Private results accumulate across grants. Match cutoff PCs to the disassembled reduce_block (C) or histogram_block (Rust).

| Application | Final private_result for N completed blocks | Success status |
| --- | --- | ---: |
| crc | u32 CRC-32/ISO-HDLC across all 4096N bytes | 17 |
| histogram | 16 u64 bins, each 256N | 29 |
| energy | u64 sum of squared (sample−128), 22370304N | 43 |
| short_report | u32 count of samples ≥240, 256 in one block | 53 |
| block_yield | u32 sum over three blocks, 1566720 | 59 |

For each slot, independently require private_bytes == 4096*N and N positive; for the long streams require N ≤ 1048576. Recompute the actual CRC in bounded host memory:

~~~python
import zlib
def expected_crc(n):
    ramp = bytes(range(256)) * 16
    crc = 0
    for _ in range(n):
        crc = zlib.crc32(ramp, crc)
    return crc
~~~

| Deployment in slot order | Check |
| --- | --- |
| crc 1,2; histogram 1,2 | First eight winners alternate 0,1. |
| crc 1,4; histogram 1,4; also reverse ELFs with same tuples | First eight slots are 0,1,idle,idle twice. |
| crc 1,2; histogram 1,2; energy 1,8 | Capacity 9/8 rejected before operational grants. |
| crc 1,4; histogram 1,4; energy 1,2 | First four winners 2,0,1,2 and three independent products. |
| block_yield 2,8; short_report 1,4 | Short peer checks and exits. Surviving caller computes one block, sleeps to a 125 ms absolute target, computes again, yields, then finishes its third block. Read request_ns/observed_ns (u64). |
| block_yield 2,8; histogram 1,4 | Observe forfeited tails, a real blocked-to-ready admission and both private products. |

Use the first plain and externally traced positions, with the default clock, 1 ms guard and the independent interval/owner criteria in the [CBS design](../../docs/native-cbs-applications.en-ko.md). Count any naturally late cutoff from the actual physical capture.
