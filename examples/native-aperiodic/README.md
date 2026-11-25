# Native aperiodic server and background applications

From the repository root in Linux or WSL, build each native ELF once. Use the same binaries and empty extra argument vectors for all four configurations.

~~~sh
mkdir -p output/aperiodic-inputs
out=output/aperiodic-inputs
sh sdk/native-aarch64/build.sh examples/native-aperiodic/service.c "$out/service.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-aperiodic-rust/Cargo.toml "$out/background.elf"
package() {
    name=$1
    policy=$2
    shift 2
    python3 tools/build_native_el0.py --out "$out/$name" --policy "$policy" \
        --server-slot-ms 50 --server-budget 2,5 --elf "$out/service.elf" \
        --peer-elf "$out/background.elf" "$@"
}
package polling polling
package deferrable deferrable
package sporadic-two sporadic --server-credits 2
package sporadic-one sporadic --server-credits 1

qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel "$out/polling/kernel.elf"
~~~

Both workers read a process-private 256-byte ramp containing 0 through 255. Each publishes its cumulative product and private_blocks after every complete ramp. The published timing unit is a batch of 4096 complete ramps (1 MiB), counted in private_batches. Each reads the SDK monotonic clock once per completed batch and exits at the first batch meeting its wall-time criterion. Both retain the first and final samples in start_ns/end_ns. All named numerical outputs are u64.

* Service: initial_checksum must be 32640. request_ns equals start_ns + 125000000. wake_ns is the first SDK clock sample after sleep returns and must reach that request. After waking, square_block processes each ramp. At exit 41 require end_ns − start_ns ≥ 650000000 and private_result = 5559680 × private_blocks.
* Background: density_block computes a histogram of the number of set bits per byte. At exit 42 require end_ns − start_ns ≥ 900000000 and private_result[0..9] equal to private_blocks times [1,8,28,56,70,56,28,8,1].
* For both processes check private_blocks = 4096 × private_batches > 0. To show progress in individual grants, inspect each private mapping between observations; a final nonzero count establishes only the overall product.

With Q/P = 2/5 in units of 50 ms slots, inspect grant cells 0 through 9. The expected sets are {0,5,6} polling, {0,3,5,6} deferrable, {0,3,5,8} sporadic with two credits, and {0,3,8,9} sporadic with one. Match the actual sleep to cell 2, each grant to available quota, and each sporadic replenishment to its due index and amount. Preserve one first plain boot per configuration and one separate externally traced sporadic-two boot (add -d int -D with a new file). Apply the [native aperiodic design](../../docs/native-aperiodic-applications.en-ko.md) to the observed journal, frames, and final products.
