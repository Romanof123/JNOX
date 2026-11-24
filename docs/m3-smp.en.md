# Two-CPU scheduling and migration

[한국어](m3-smp.md) · [Change #47](https://github.com/Romanof123/JNOX/issues/47)

CHG-019 connects POL-013 and REQ-002/005/008/023 to DES-003/004/012, IMP-017 and VER-002/005/008/023. This configuration has two Cortex-A72 virtual CPUs on QEMU virt, GICv2, a physical generic timer, 128 MiB RAM and multithreaded TCG. Both jobs in the initial release become ready in one transaction.

| Case | Action and expected result |
| --- | --- |
| Bootstrap | CPU0 boots at EL1 and starts affinity 1 through PSCI CPU_ON64. Both CPUs enable identity translation using the same inner-shareable Normal RAM and Device MMIO attributes. CPU1 has a private bootstrap stack. Both CPUs observe a bidirectional atomic handshake before dispatch. |
| Partitioned | M and P0 can run only on CPU0. P1 can run only on CPU1. M and P1 must both enter before either can finish. P0 runs on its home after M. |
| Global | M may use either CPU. CPU1 finishes P1 and attempts to claim the running M. The attempt must fail. A physical timer IRQ on CPU0 interrupts M, saves its frame and readies pinned P0. P0 on CPU0 and M on CPU1 must make progress. Match the interrupted PC to the restored PC and check M's stack, registers, TLS and progress. |
| Clustered | Use singleton logical clusters. CPU1 attempts to claim a ready M restricted to CPU0, while CPU1 is free. It must be rejected. P0 runs on CPU0 and M resumes in its home cluster. |
| Ownership and accounting | Each lease belongs to one task, CPU and grant within one ledger lifetime. A lease retires once. Accounting adds one observed generic-counter interval from grant to retirement. Double retirement and overflowing charges must preserve the existing state. Preserve the outgoing frame on the task stack, then switch to a CPU-private handler stack before publishing it under the coordinator lock. |

The bench maps a single privileged 2 MiB RAM block containing the linked image, stacks and tables. This bootstrap map permits privileged writes and instruction fetch in that block. Device mappings cover only the GIC distributor, CPU interface and UART pages. The product memory-protection design requires separate verification.

## Reproduction and verdict

From the repository root, build with the pinned Rust toolchain and run one guest at a time.

```sh
(cd kernel && cargo build --locked --release --features smp-bench -j1)
python3 tools/qemu_boot.py --profile smp --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

A fresh verifier sees this contract and the frozen source/image identifiers first. Record the first verdict before looking at implementation or prior verdicts. Retain UART, QEMU exception events, per-CPU identities, frame/lease records, counter frequency and release/claim timestamps from the same run. Check the complete transcript, a 100 ms quiet window and host termination. Reconstruct the timeline independently, allowing legal interleavings. Host coverage exercises invalid mixed releases, affinity, competing claims, copied leases, checked addition and a placement oracle.

Virtual counter deltas describe the selected run. Physical migration delay, shared-cache interference under defined loads, target-specific acceptance thresholds and safety approval remain open under POL-013. Board access is pending. [Scheduling catalogue](scheduling-policies.en.md) · [Board tracking](https://github.com/Romanof123/JNOX/issues/34)

The smp_audit ELF symbol contains 140 little-endian 64-bit words. Words 0–4 give version, completed epochs, row count and raw CPU affinities. Words 5–18 give two rejected claims. Words 19–30 give source/destination, saved/selected PC, saved/live SP and register witnesses. Words 31–39 and 40–48 hold service and CPU masks by epoch/task. Each of the eleven rows in 49–136 records epoch, task, CPU, lease number, start, end, duration and raw affinity. Words 137–139 give counter frequency, source IRQ entry and destination callback time. Epochs 0/1/2 are partitioned/global/clustered. Tasks 0/1/2 are M/P0/P1. To retain the journal and QMP/IRQ evidence, set SMP_CAPTURE_DIR to a new directory and pass a Linux executable wrapper running python3 tools/smp_capture.py "$@" to the observer with --qemu.

For the serialized TCG control use --profile smp-single. It uses the same two guest CPUs and image with tcg,thread=single. Both observers use a five-second UART deadline. An idle processor or a processor waiting for its peer issues the AArch64 YIELD hint. Record this mode independently. [QEMU 6.2 AArch64 hint implementation](https://github.com/qemu/qemu/blob/v6.2.0/target/arm/translate-a64.c#L1324-L1333)
