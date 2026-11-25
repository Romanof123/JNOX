# Native SMP register preservation control

From the repository root, build both ordinary SDK ELFs and package them once.

~~~sh
sh examples/native-pi4-smp/abi/build.sh abi-jobs
python3 tools/build_native_el0.py --board pi4 --policy smp-partitioned     --elf abi-jobs/odd.elf --peer-elf abi-jobs/even.elf     --cpu-mask 1 --peer-cpu-mask 2 --priority 2 --peer-priority 2 --out abi-pair
sha256sum abi-jobs/*.elf
for job in odd even; do
    aarch64-linux-gnu-objdump -d abi-jobs/$job.elf > abi-jobs/$job.dis
    aarch64-linux-gnu-nm -nS abi-jobs/$job.elf > abi-jobs/$job.symbols
done
python3 tools/pi4_native_smp_capture.py abi-pair abi-first-plain
python3 tools/pi4_native_smp_capture.py abi-pair abi-first-traced --trace
~~~

Save the manifest, ordered hashes, maps and linked loop instructions before either first launch. Both jobs use the normal ELF startup and return through main. Their 33,554,432 iterations sum consecutive positive odd or even integers in x6 and d1. x4 counts completed increments; interpret intermediate values using the exact instruction at the sampled PC. The loop spans [smp_abi_loop, smp_abi_loop_end). The private 128-byte smp_abi_result contains 16 little-endian u64 words in the order declared in client.c. Phase 1 marks entry into work, 2 marks completion of register readbacks, and 3 marks successful C validation. Compare both final sums to 1,125,899,906,842,624 (odd, exit 12) or 1,125,899,940,397,056 (even, exit 77). Error exits 81 and 82 preserve the evidence. Each ELF aligns smp_abi_tls to 32 bytes, choosing offset 0 or 8; check the corresponding actual VA and private PA. Both clients only read TPIDRRO_EL0. Terminal measurements alone do not establish interrupt or overlap coverage. Apply the [frozen first-position criteria](../../../docs/pi4-native-smp-abi-control.en-ko.md).
