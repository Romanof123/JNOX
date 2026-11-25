# Recurring native deadline service

Build three independent binaries from the repository root on Linux or WSL. A is the short checksum. B is the original Rust nine-bin histogram using its default dataset. The service is an ordinary C application with its own writable sample buffer.

~~~sh
mkdir -p output/deadline-inputs
out=output/deadline-inputs
sh sdk/native-aarch64/build.sh examples/native-llf/checksum.c "$out/checksum.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml "$out/histogram.elf"
sh sdk/native-aarch64/build.sh examples/native-deadline-service/service.c "$out/service.elf"

package() {
    python3 tools/build_native_el0.py --out "$out/$1" --policy "$2" \
        --elf "$out/checksum.elf" --job 0,12,1 \
        --peer-elf "$out/histogram.elf" --peer-job 0,13,4 \
        --deadline-service-elf "$out/service.elf" --deadline-service-cells "$3" \
        --deadline-switch-cap 6
}
package edf edf 1,3
package llf llf 1,3
package alternate edf 2,4
~~~

The packager supplies the service calendar as argv[1]. The same service ELF accepts any nonempty increasing subset of cells 0 through 15, including adjacent indices. On each activation it fills its 64-element private array service_samples, computes the sum of squares in service_reduce, publishes one receipt, then requests the next ordinal or exits 41 after the final receipt.

For UART and optional external exception collection, freeze the ELF hashes, packages, QEMU version and commands before boot. Use a separate output directory for each position.

~~~sh
capture() {
    name=$1
    package_name=$2
    shift 2
    mkdir "$out/$name" || return 1
    timeout 120s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
        -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
        -serial "file:$out/$name/uart.bin" "$@" -kernel "$out/$package_name/kernel.elf"
    status=$?
    echo "$status" > "$out/$name/host-status"
}
capture edf-plain edf
capture edf-traced edf -d int -D "$out/edf-traced/exceptions.log"
capture llf-plain llf
capture llf-traced llf -d int -D "$out/llf-traced/exceptions.log"
capture alternate-plain alternate
~~~

A host timeout is recorded as status 124 and is separate from the signed application exits. For scored positions also retain the terminal RAM and exception journal through the acquisition workflow. Inspect the service symbols in slot 2's mapping. The little-endian service_receipts array contains 16 rows of 32 bytes. In each row, u32 fields at offsets 0/4/8/12/16/20 are ordinal, cell, first sample, last sample, count, and reserved zero. Offset 24 contains the u64 sum of squares. service_receipt_count (u32) commits the valid row prefix. service_wait_ordinal[i] (i32) records the successful return that resumed ordinal i for i>0.

For row i require first=64i+1, last=64i+64, count=64 and cell=calendar[i]. Independently compute the product as F(last)−F(first−1), where F(n)=n(n+1)(2n+1)/6. Both two-activation calendars must produce 89440 and 617824, with their own cell indices. Check A's 16 blocks, sum 8355840 and exit 17; B's 67108864 bytes, all nine bins equal to 262144 × C(8,k) and exit 197. Apply the [recurring-service design](../../docs/native-deadline-recurring.en-ko.md) for the two admission refusals per policy, chronological switch accounting and interrupted-job evidence.

Replay the terminal UART record for a captured position with the public analysis command:

~~~sh
python3 tools/replay_native_deadline_service.py "$out/edf-plain/uart.bin"
~~~
