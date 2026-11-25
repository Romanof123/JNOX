# Native EDF sensor jobs

Build from the repository root in Linux or WSL. Use a fresh output path per package.

~~~sh
for app in batch urgent sleep_first sleep_last midgrid; do
    sh sdk/native-aarch64/build.sh examples/native-edf/"$app".c "$app".elf
done
sh sdk/native-aarch64/build-rust.sh examples/native-edf-rust/Cargo.toml histogram.elf
python3 tools/build_native_el0.py --out edf-three --policy edf \
    --elf batch.elf --job 0,8,2 \
    --peer-elf histogram.elf --peer-job 0,12,8 \
    --peer-elf urgent.elf --peer-job 1,4,1
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel edf-three/kernel.elf
~~~

Each 4096-byte block contains 16 permutations of bytes 0 to 255. All three jobs publish completed blocks in private_progress (u32). Map the symbols in each packaged ELF to its own address space.

| ELF | private_result | Work symbol | Exit |
| --- | --- | --- | ---: |
| batch | u64 sum, 522240 times completed blocks. The first qualifying block ends at least 160 ms after start. At most 65536 blocks. | batch_reduce | 17 |
| histogram | 16 u32 bins, each 32768 after 128 blocks | histogram_reduce | 29 |
| urgent | u32 count of values at least 240, 2048 after eight blocks | urgent_reduce | 43 |

With 100 ms grid units, the three-job admission model finishes urgent, batch and histogram at indices 2, 3 and 11. Compare measured completion with each job’s own deadline. Compare the release-interrupted PC to batch_reduce and record the actual resumed frame and continued work.

| Control | Package order and tuples (release, deadline, cost) | Expected observation |
| --- | --- | --- |
| Equal initial deadlines | batch (0,8,2), urgent (0,8,1) | Package order starts batch. Sampled arbitration retains the live incumbent. |
| Equal later arrival | batch (0,8,2), urgent (1,8,1) | Batch retains CPU at the grid-1 arrival. |
| Initially idle | urgent (1,4,1) | First entry after grid 1. |
| Rejected demand | urgent (0,1,2) | Admission rejects before entry. |
| Estimated-cost overrun | batch (0,8,1) | Compare service with Q, then check later progress and exit. |
| Model equality and measured miss | batch (0,1,1) | Exact model completion at grid 1; measure actual exit after the deadline. |
| Equal yields and wakes | sleep_first (0,10,1), sleep_last (0,10,1) | First yields, last requests 140 ms, first requests 40 ms. Both block, first wakes then exits; last wakes, calls elapsed sleep and yields alone. |
| Mid-grid wake | midgrid (0,5,1), batch (0,8,2) | Earlier job requests 40 ms. Batch remains active after the mid-grid wake until the next sampled arbitration. |

Sleep clients export request_ns and observed_ns (u64 absolute monotonic nanoseconds), private_progress (u32, 1 before sleep and 2 after a successful clock read) and private_result (u32). Expected final results and statuses are 53, 59 and 61 for sleep_first, sleep_last and midgrid respectively. Each checks its original private value after resumption. Operand-only and numeric controls are described in the [EDF design](../../docs/native-edf-applications.en-ko.md).
