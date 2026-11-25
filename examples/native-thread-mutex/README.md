# Shared sensor aggregation with a native mutex

Build the C entry point and Rust callback into one ordinary AArch64 ELF. Use the public SDK script from the repository root on Linux/WSL; the Cargo build script compiles sensor.c with clang for aarch64-none-elf and passes its object to the Rust linker. The SDK supplies the process startup, C services, Rust library and linker script.

~~~sh
mkdir -p output/mutex-input
out=output/mutex-input
sh sdk/native-aarch64/build-rust.sh examples/native-thread-mutex/Cargo.toml "$out/sensor.elf"
python3 tools/build_native_el0.py --out "$out/coop" --threads 2 --elf "$out/sensor.elf"
python3 tools/build_native_el0.py --out "$out/rr" --threads 2 --policy rr --quantum-ticks 625000 --elf "$out/sensor.elf"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$out/coop/kernel.elf"
~~~

Keep the ELF identical across packages. Freeze the build, ELF disassembly, default clock and acquisition settings on ext4 before boot. Capture the first plain then externally exception-traced cold boot in each policy; use a fresh log for each QEMU -d int -D invocation.

Main initializes sensor_samples to 1 through 256 and locks sensor_mutex before creating the Rust callback sensor_worker. The worker tries once, announces its attempt using the release/acquire worker_phase flag and calls blocking lock. Main yields while holding the mutex, sums the first 128 samples and unlocks. The worker acquires the same mutex and sums the second 128. Independently check all three consecutive u64 fields of sensor_total: count 256, sum 256×257/2 = 32896 and sum of squares 256×257×513/6 = 5625216. joined_aggregate must equal the address of sensor_total and process status must be 0. Observe the blocked worker frame and direct handoff in the scheduler journal.

worker_try must record EBUSY (16), worker_lock success (0), main_errno_after EINVAL (22), and worker_errno_after ENOSYS (38). Main records its immediate post-unlock try in main_try_after. In the cooperative position require EBUSY after the direct transfer. For RR correlate this result with the actual order of unlock, worker return and retry. If the worker already finished and main succeeds, main releases the reacquired lock.

For independent controls, add --arg sleep-owner or --arg controls to either package command using a fresh output directory. The sleeping owner requests a monotonic absolute deadline 20 ms ahead while holding the mutex. Inspect sleep_target_ns and sleep_observed_ns (u64) plus real idle and IRQ return. In controls mode check self_try=EBUSY, busy_destroy=EBUSY and nonowner_error=EPERM (1); the main thread also destroys, reinitializes, locks, unlocks and destroys the same object at its original address. The aggregate and success exit are identical in both cases. The [native mutex design](../../docs/native-thread-mutex.en-ko.md) specifies the remaining lifecycle and evidence criteria.
