# Round-robin native thread streams

From the repository root in Linux or WSL, build the ELF once and package it for both dispatch policies.

~~~sh
mkdir -p output/thread-rr
out=output/thread-rr
sh sdk/native-aarch64/build.sh examples/native-thread-rr/stream.c "$out/stream.elf"
python3 tools/build_native_el0.py --out "$out/rr" --elf "$out/stream.elf" --threads 2 --policy rr --quantum-ticks 625000
python3 tools/build_native_el0.py --out "$out/coop" --elf "$out/stream.elf" --threads 2
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$out/rr/kernel.elf"
~~~

Use the default clock. Freeze the ELF, disassembly, versions, commands, journal acquisition and timeout before boot. Retain first RR plain then externally exception-traced cold boots; repeat cooperatively using coop/kernel.elf. For exception tracing use a separate -d int -D logfile per run.

Each 4096-byte ramp contains sixteen permutations of 0 through 255. Main sums 522240 per block. The worker adds 256 per high-nibble bin per block. In the default run both complete 16384 blocks and return exit 17 after joining. Inspect main_blocks and worker_blocks (u32), main_sum (u64, 8556380160), all sixteen u64 worker_bins (4194304 each), and joined_result equal to the address of worker_job. Each progress counter is published after its block product. Main accesses the worker's live output only after join.

Match the two successive involuntary changes to main_reduce and worker_reduce in the linked disassembly and their separate stacks. Check subsequent progress and both complete products. With cooperative dispatch the worker first runs after main completes its own blocks and blocks in join.

Use the same ELF for the following RR controls, adding --arg MODE and a fresh output path.

| MODE | Work and observation |
| --- | --- |
| early-exit | One worker block; main continues through 16384 blocks. Check actual post-exit service and absence of slice-only switches. |
| early-wake / late-wake | Main yields after create; worker requests a 3 ms / 25 ms absolute wait. Both complete 16384 blocks. Compare recorded sleep_target_ns and sleep_observed_ns (u64) with actual slice expiry and decisions. |
| late-create | Main completes 16384 blocks before creating the worker and another 16384 afterwards. Compare the original solo turn expiry with time of create. |
| idle-join | Main completes one block and joins. Worker requests an absolute 20 ms sleep, counts one block, returns its descriptor. Check real idle and timer-originated resume. |
| errno | Normal input sizes. Main sets EINVAL with clock_gettime; worker sets ENOSYS with close. Check main_errno_after and worker_errno_after, both products, and asynchronous switch records. |

For worker-slot reuse and stale-handle control, package examples/native-threads/sensor.c with the same threaded RR options; the [existing workload](../native-threads/README.md) records its two handles and stale result. Use the [thread RR design](../../docs/native-thread-round-robin.en-ko.md) to evaluate recorded timing and ownership.
