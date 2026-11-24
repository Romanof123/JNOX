# Mixed-criticality budget enforcement

[한국어](m3-mixed-criticality.md) · [Change #51](https://github.com/Romanof123/JNOX/issues/51)

CHG-022 · POL-015 · REQ-002/005/023 → DES-003/012 → IMP-020 → VER-002/005/023. The finite Cortex-A72 virt configuration has one CPU, four task slots, one GICv2 physical timer and six service slots. Q is the integer part of counter frequency divided by 20. Slot boundaries are anchored at origin plus an integer multiple of Q. Larger priority wins.

| ID | Criticality | LO/HI budget in slots | Priority | Relative deadline in slots |
| --- | --- | --- | ---: | ---: |
| 0 | LO | 2 | 1 | 3 |
| 1 | HI | 1/3 | 4 | 4 |
| 2 | LO | 2 | 2 | 3 |
| 3 | HI | 1/1 | 3 | 5 |

Start job 0. At tick 1 complete it and fill all four slots including a fresh 0. Job 1 consumes its LO allowance. At tick 2 the remaining requested work is 2, 3, 2, 1 units for task IDs 0–3. Job 1’s next service requires HI mode. Drop both LO jobs before further service, reject a new LO release and reject early reset while a surviving HI job is active. Job 1 may consume three slots in total. Reject the fourth grant and retire the job on budget exhaustion. The surviving job 3 must complete its computation by its deadline. At quiescence reject an unapproved reset, then accept the approved reset and execute a newly released LO job. Expected slot owners are 0, 1, 1, 1, 3, 0.

Record six real IRQs, physical deadline and IRQ sample at each slot, PC in the selected job’s work loop, private stack, execution progress and result. Check a shared-clock ordering from the transition boundary through sampling, state commit and execution of the surviving HI context. Count all four occupied slots at the maximum carry-in in this fixture. This configuration supplies one discrete budget model and a virtual transition observation. Product timing bounds, maximum product workload and system fault response require separate evidence.

Build in kernel with `cargo build --release --locked --features amc-bench -j1`. From the root run `python3 tools/qemu_boot.py --profile amc --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel`. On the first run capture QEMU -d int in a separate file and preserve the complete UART from the same process. The terminal line is AMC PASS. Match each logged ELR to the frozen ELF’s amc_lo_initial, amc_hi, amc_survivor and amc_lo_fresh begin/end ranges and verify actual exception returns. The HI counter samples must obey IRQ ≤ commit ≤ first resumed-work sample < next compare. Report that sample minus the tick-2 boundary.
