# Native LLF crossover jobs

Run from the repository root in Linux or WSL with the native C and Rust SDK toolchains.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-llf/checksum.c checksum.elf
sh sdk/native-aarch64/build-rust.sh examples/native-llf-rust/Cargo.toml histogram.elf
for policy in llf edf; do
    python3 tools/build_native_el0.py --out "$policy-pair" --policy "$policy" \
        --elf checksum.elf --job 0,6,1 --peer-elf histogram.elf --peer-job 0,7,3
done
timeout 120s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
    -serial stdio -kernel llf-pair/kernel.elf
~~~

Both processes independently fill 4096 bytes with (17*i+3) modulo 256. Each byte value occurs sixteen times per block, giving a sum of 522240 and 256 occurrences in each high-nibble bin. In each private mapping, private_blocks is the u32 count of completed blocks. Results are published before incrementing that count.

| ELF | Reduction symbol | Final private_blocks | Full private_result | Exit |
| --- | --- | ---: | --- | ---: |
| checksum | checksum_reduce | 16 | u64 sum 8355840 | 17 |
| histogram | histogram_reduce | 131072 | Sixteen u64 bins, each 33554432 | 29 |

Keep the same ELFs for LLF and EDF. Retain the first LLF plain and externally traced positions, then EDF plain and traced. Use the [LLF design](../../docs/native-llf-applications.en-ko.md) for the sampled crossover, frame restoration, progress and deadline criteria. Single-job controls use checksum (0,4,1), histogram (0,1,1), and rejected demand (0,1,2).

## Optional 64 MiB bit-count histogram

To process 64 MiB with the existing [Rust bit-count histogram](../native-weighted-rust/src/main.rs), build its default input without the extended-input feature. Keep the checksum ELF from the commands above and use a new package directory:

~~~sh
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml histogram-64m.elf
python3 tools/build_native_el0.py --out llf-pair-64m --policy llf \
    --elf checksum.elf --job 0,6,1 --peer-elf histogram-64m.elf --peer-job 0,7,3
timeout 120s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
    -serial stdio -kernel llf-pair-64m/kernel.elf
~~~

The default worker processes 262,144 blocks of 256 bytes. Its nine private bins count bytes with k set bits, with expected bin k equal to 262,144 × C(8,k) and expected exit 197. The original 512 MiB ELF above counts high nibbles into sixteen bins and exits 29. Both use the same job tuple. Their measured deadlines are recorded separately.

On the verified QEMU configuration the original 512 MiB histogram missed its actual deadline. Both jobs in the 64 MiB pair completed on time in the supplemental plain and traced boots. [Measured results](../../docs/evidence/evd-121.en.md)
