# Mutex scheduling under contention

[한국어](m1-mutex.md) · [Change #49](https://github.com/Romanof123/JNOX/issues/49)

CHG-020 traces POL-014 and REQ-002/005/023 through DES-003/012, IMP-018 and VER-002/005/023. The virtual bench uses one Cortex-A72, GICv2, a physical generic timer and four privileged task contexts. BASE, LOW, MEDIUM and HIGH have priorities 1, 2, 5 and 9. Higher numbers win.

| Case | Expected decisions |
| --- | --- |
| Direct donation | LOW owns X. Release HIGH and MEDIUM together. When HIGH attempts X it blocks. Select the boosted LOW ahead of the ready MEDIUM. Unlock hands X to HIGH and restores LOW’s base priority. Check HIGH’s return from its saved acquire call and eventual unlock. |
| Transitive donation and timeout | BASE owns Y. LOW owns X and blocks on Y. Release HIGH and MEDIUM. When HIGH waits on X, both LOW and BASE inherit 9. BASE runs while MEDIUM is ready. A physical timer IRQ withdraws HIGH’s wait. HIGH resumes with timeout. After HIGH exits, MEDIUM runs ahead of both former donation recipients. |
| Immediate ceilings | X has ceiling 5 and permits LOW and MEDIUM. Y has ceiling 9 and permits LOW and HIGH. LOW acquires X then Y, reaching 5 then 9 before HIGH waits. After an explicit yield HIGH blocks on Y. LOW releases only Y, transfers it to HIGH and retains priority 5 from X. Final release of X restores 2 and permits MEDIUM to run. |

One task can hold two locks and wait for at most one further eligible acquisition within the nesting bound. Release in reverse acquisition order. Reject invalid user masks and ceilings, unauthorized and recursive acquisition, cycles and non-owner unlock before changing ownership. Synchronization operations execute serially on the bench. An inheritance edge propagates the waiter’s current effective priority. An immediate ceiling boosts the owner at acquisition. These two rules retain their own meaning when a configuration uses both kinds of lock. Among eligible waiters, transfer to the highest effective priority and break ties in arrival order.

The verifier records the first launch against the public contract. Check real SVC/IRQ return addresses, thread frames, owner, waiter and ready sets, priority before and after, timer expiry and restored stacks. Retain the frozen image and trace. Physical maximum blocking, interrupt latency, product load envelope and safety approval remain open.

## Reproduction

Build from kernel with `cargo build --release --locked --features mutex-bench -j1`. From the repository root use `python3 tools/qemu_boot.py --profile mutex --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel`. The expected lines after the two boot markers are MUTEX PI TRANSFER, MUTEX CHAIN TIMEOUT, MUTEX CEILING NESTED and MUTEX PASS. Allow five seconds and require 100 ms of silence before host stop.

The ELF symbol mutex_witness is 1,304 bytes. The three little-endian u64 values at its start give row count, timer deadline and IRQ counter. Each 32-byte row contains the outgoing ELR and frame address as two u64 values, followed by sixteen unsigned bytes in order: epoch, operation, argument, caller, selected task, ready mask including the selected task, X and Y wait masks, X and Y owners, four effective priorities in task-ID order, woken task and interrupt ID. Value 255 denotes absence. Operations 1 through 6 denote acquire, release, activate, yield, exit and timer expiry. There are 40 slots. Expect 8, 14 and 12 committed transitions. The IRQ must fall inside the declared BASE work loop. Collect this table, the raw exception log and UART from the same launch using the QMP wrapper, then map PCs against the frozen ELF. Use an executable Linux wrapper running `python3 tools/mutex_capture.py "$@"` with a new `MUTEX_CAPTURE_DIR` and supply it via `--qemu`.

[Independent verification](evidence/evd-018.en.md)
