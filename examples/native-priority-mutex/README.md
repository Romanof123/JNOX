# Four-thread priority and mutex examples

Build one ELF containing the ordinary C process and a Rust reduction, then reuse its exact bytes for every package. From the repository root on Linux:

~~~sh
mkdir -p output/priority
sh sdk/native-aarch64/build-rust.sh examples/native-priority-mutex/Cargo.toml output/priority/priority.elf
python3 tools/build_native_el0.py --out output/priority/chain --elf output/priority/priority.elf --arg chain --threads 4 --policy fifo --thread-priorities 1,2,3,4
python3 tools/build_native_el0.py --out output/priority/rr --elf output/priority/priority.elf --arg rr --threads 4 --policy rr --thread-priorities 1,2,2,4 --quantum-ticks 625000
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel output/priority/chain/kernel.elf
~~~

Choose the matching row below and change the output directory for each package. All times use the SDK monotonic clock. T is a single absolute target 40 ms after the setup sample. The application exits 99 and publishes the source line in failure_line if an assertion fails, including a start that misses T. A successful run returns 0 and leaves stages[0]=3.

| Argument | Package | Work and observable order |
|---|---|---|
| chain | FIFO 1,2,3,4 | BASE owns Y. LOW owns X and waits for Y. MEDIUM and HIGH sleep until T. HIGH then waits for X while BASE performs its reduction. The receiver checks the entire preceding product under Y or X. Transcript 0,1,3. |
| timeout | FIFO 1,2,3,4 | Same chain. BASE sleeps with Y until T+40 ms, HIGH waits on X until T+20 ms, MEDIUM does independent work. Require timeout_result=ETIMEDOUT, transcript 0,1 and coherent joined products. Observe the surviving LOW→BASE donation and MEDIUM service in the kernel journal. |
| protect | FIFO 1,2,3,4 | Main takes X (mask 3, ceiling 2), then Y (mask 15, ceiling 4). Explicit priority-3 MEDIUM becomes ready inside the nested section. Releasing Y permits MEDIUM to run. A third protocol lock and out-of-order unlock are refused. MEDIUM tries X as an undeclared user. Transcript 0,2. |
| rank | FIFO 1,2,3,4 | While main sleeps with X, LOW queues at T, MEDIUM at T+20 ms. Main releases after T+50 ms. The higher-rank waiter receives X first. Transcript 0,2,1. |
| fifo12 / fifo21 | FIFO 1,2,2,4 | Reverse the two waiters' release times across packages. Equal-base worker slots are allocated in creation order. Expected transcripts are 0,1,2 and 0,2,1 respectively. These and rank also check nonowner unlock, reaped worker reuse, stale join, and a stale mutex token at the same reinitialized address. |
| cycle | FIFO 1,2,3,4 | Main owns Y. LOW takes X and blocks on Y. Main's lock of X must return EDEADLK. Successful subsequent handoff yields transcript 0,1. |
| rr / fifo | Corresponding policy, 1,2,2,4 | Equal-rank LOW and MEDIUM wake at T for fixed independent reductions. Explicit FIFO HIGH wakes at T+25 ms for a short reduction. In RR use Q=625000 ticks. Recover actual timer changes, PCs, residual entitlement and FIFO retention from the journal and external trace. |

The input ramp is 0 through 255. Each completed block contributes 32,640 and 5,559,680 to the two wide checksums. The named Rust function reduce_ramps publishes all three u64 fields in products[role]: completed blocks, sum and squared sum. LARGE is 524,288 blocks (134,217,728 input bytes), SMALL is eight. Chain/timeout use LARGE for BASE and MEDIUM, SMALL for LOW and HIGH. Cycle uses SMALL for BASE and LOW. Protect uses SMALL for BASE and MEDIUM. Waiter cases use SMALL for BASE and both waiters. The RR/FIFO competition uses LARGE for both equal peers and SMALL for HIGH. Joined pointers identify the corresponding product records. The reuse probe returns the original LOW product address without recomputing it.

Freeze one ELF and its disassembly, package arguments, the above timings and inputs before the first cold boots. Capture plain then separately with external exception tracing for chain, timeout and protect. Run each remaining control once. Correlate the effective ranks, real block/timeout/transfer, productive PCs, fresh versus residual quantum and orderly restoration against the private products, stage flags, joined pointers and transcript. Retain any missed ordering or unobserved preemption in its original position. For two-thread NULL/default mutex and singleton compatibility use their existing unchanged packages. [Frozen acceptance contract](../../docs/native-thread-priority-protocols.en-ko.md)
