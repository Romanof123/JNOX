# Periodic sensor batches

Build from the repository root with the public native SDK in Linux or WSL.

~~~sh
for app in crc crc_overrun yield_report sleep_backlog unsupported; do
    sh sdk/native-aarch64/build.sh examples/native-periodic-job/"$app".c "$app".elf
done
sh sdk/native-aarch64/build-rust.sh examples/native-periodic-job-rust/Cargo.toml histogram.elf
for policy in rm dm; do
    python3 tools/build_native_el0.py --out "$policy-batches" --policy "$policy" \
        --elf crc.elf --periodic 0,100000000,90000000 \
        --peer-elf histogram.elf --peer-periodic 0,120000000,40000000
done
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel rm-batches/kernel.elf
~~~

Use the same ELFs for both policies. Each report producer exports volatile u32 arrays report_result, report_bytes and report_sequence, and scalar report_count. CRC and alarm reports contain three scalar products; the histogram has three consecutive rows of sixteen bins. Expected final sequence is [1,2,3] and count is 3. All three producers wait twice and then exit. Compare per-process RAM with the following values.

| ELF | Bytes per generation | Products for g=0,1,2 | Exit |
| --- | --- | --- | ---: |
| crc | 16384, 16640, 16896 | Reflected CRC-32 0xe81722f0, 0xb03637e8, 0x82e8647b | 17 |
| histogram | 8192, 8448, 8704 | Each of the 16 bins equals 512, 528, 544 respectively | 29 |
| yield_report | 256 each | Cumulative alarm totals 16, 32, 48, held in a local across waits and yields | 43 |

For each overrun position replace crc.elf with crc_overrun.elf at the same tuple. Generation 1 requires at least one extra checksum ramp and an observed duration greater than 110 ms from generation start. The app fails explicitly at 262144 ramps if the interval is still incomplete. Read extra_start_ns and extra_end_ns (u64), report_bytes[1] and the full CRC from RAM. For every CRC generation check n = report_bytes[g] / 256 with zero remainder, and independently compute:

~~~python
import zlib
def expected_crc(g, n):
    return zlib.crc32(bytes((i + g) % 256 for i in range(256)) * n)
~~~

Map crc_ramp and histogram_batch PCs in the packaged ELFs. The CRC worker publishes active_blocks after each completed ramp. Retain the actual counts, interruption point, anchored releases, deadline and overlap evidence for each plain and traced position.

| Additional input configuration (phase, period, deadline in ns) | Expected observation |
| --- | --- |
| yield_report alone, (20000000,100000000,90000000) | Initial idle; delayed first entry, two wait returns, three reports, sole yields. |
| yield_report then crc, both (0,100000000,90000000) | Equal policy keys; first slot yields to the waiting peer. Both return the tabled products. |
| Reference pair plus yield_report at (10000000,140000000,80000000) | All three independently built processes complete their three reports. |
| sleep_backlog alone, (0,20000000,15000000) | Generation zero requests a 65 ms absolute sleep. Later releases accrue while idle, then exit 53 commits generation zero and cancels the released backlog. Read request_ns, observed_ns (u64) and report_count=1. |
| unsupported alone with default cooperative policy | Wait returns -1 with ENOTSUP; read recorded_result and recorded_errno, exit 59. |

Apply the evidence and timing criteria in the [periodic process design](../../docs/native-periodic-applications.en-ko.md).
