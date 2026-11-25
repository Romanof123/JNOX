# Cyclic energy and bit-density workers

From the repository root in Linux or WSL, build two independent AArch64 binaries with the public SDK. Use the same ELFs for both tables and the sleep control.

~~~sh
mkdir -p output/cyclic-inputs
out=output/cyclic-inputs
sh sdk/native-aarch64/build.sh examples/native-cyclic/energy.c "$out/energy.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-cyclic-rust/Cargo.toml "$out/density.elf"
package() {
    name=$1
    windows=$2
    shift 2
    python3 tools/build_native_el0.py --out "$out/$name" --policy partition \
        --partition-windows "$windows" --elf "$out/energy.elf" --peer-elf "$out/density.elf" "$@"
}
package table-p a:100,b:50,idle:50
package table-q b:50,idle:50,a:100,b:50
package sleep-p a:100,b:50,idle:50 --peer-arg sleep-160

qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel "$out/table-p/kernel.elf"
~~~

Freeze inputs, manifests, ELF disassemblies, QEMU argv, counter frequency and acquisition settings before capturing P plain/traced, Q plain/traced, then P sleep plain. For external exception tracing add -d int -D with a fresh file path. Keep both private mappings and all first results for the [cyclic application criteria](../../docs/native-cyclic-applications.en-ko.md).

At main entry each worker samples CNTPCT_EL0 with ISB and reads CNTFRQ_EL0. first_counter, observed_frequency and final_counter are u64. Both process full 256-byte ramps of 0..255 and check time at each completed block. They stop at the first such boundary at least 900 ms after first_counter. B's optional sleep calls the ordinary monotonic SDK with a 160 ms absolute wait; request_ns and observed_ns are u64 monotonic nanoseconds.

| Worker | Inputs and expected private outputs | Exit |
| --- | --- | ---: |
| A, block_energy | private_blocks is a u64 count N. private_result is a u64 equal to N × sum(x² for x=0..255) = N × 5559680. | 41 |
| B, density_block | private_blocks is a u64 count N. private_result is nine consecutive u64 bins, N times [1,8,28,56,70,56,28,8,1]. | 42 |

At startup each process writes the first sample with completed=0 and witness=0. At block boundaries it appends when at least ceil(observed_frequency/5000) physical ticks have passed since its last appended sample. records[8192] consists of 24-byte little-endian rows with three u64 fields: counter, completed blocks and witness (A's energy or B's bin 4). record_count (u32) publishes the valid prefix after each row, and record_overflow (u32) becomes 1 if the buffer fills and another row is due. The ELF symbols are per-process. Match each sample to a return and subsequent exception in that same context before using it to bound arrival.
