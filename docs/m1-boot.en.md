# M1 AArch64 kernel boot bench

[한국어](m1-boot.md) · English

`CHG-001` introduces the first executable increment of the M1 kernel from planning baseline `4ce1e16`. `IMP-001` covers the AArch64 entry point, Rust startup, early UART output and CPU parking. The engineering question is whether the same standalone `no_std` kernel ELF reaches a defined point in Rust after each cold start on the P1 reference emulator. [M1 product scope](product-plan.en.md) · [Engineering lifecycle](lifecycle.en.md)

| Trace | Scope of this change |
| --- | --- |
| `OBJ-008` → `REQ-016` → `DES-001` | Build the Rust kernel with `core`, its startup assembly and its linker script. Review the image entry, stack, BSS bounds and unsafe operations used to reach and drive the boot UART. |
| `IMP-001` → `VER-016` | Identify the exact executable, observe it across cold starts and compare independent clean builds. Review the toolchain, dependencies, final link and the preconditions for assembly, volatile memory access and MMIO. |
| `OBJ-001`, `THR-002` | Retain the candidate identity and build provenance for review. This bench has no physical outputs. Image authentication, toolchain qualification and product safety acceptance require separate evidence. |

[System requirements](requirements.en.md) · [Microkernel design](architecture.en.md) · [Verification plan](verification-plan.en.md)

## UART contract

Use QEMU `virt` with TCG, one Cortex-A72 CPU, 128 MiB RAM and the kernel ELF supplied directly through `-kernel`. Route the first UART to the runner, disable the monitor and display, and capture emulator diagnostics separately. `READY` means that Rust observed the expected zero value in the BSS probe and completed the startup UART sequence. It grants no other service availability.

The entry code places a nonzero value in the BSS probe before clearing the BSS. Rust reads the probe through a volatile access before emitting either line. This makes the observation meaningful even if the ELF loader initially supplies zeroed memory. After both lines the CPU stays in a `WFE` loop. The complete UART stream for a successful launch is two ASCII lines in this order, each terminated by LF or CRLF.

```text
JNOX M1 BOOT
JNOX M1 READY
```

## Reproduction and verdict

From the repository root, run the following commands in a shell with the pinned Rust toolchain, `aarch64-unknown-none` target, Python 3 and `qemu-system-aarch64` available.

```sh
cd kernel
cargo build --release --locked
cd ..
python3 tools/qemu_boot.py --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

1. **Two cold starts.** Use the same ELF SHA-256 for both launches. Give each launch a fresh QEMU process and a fresh serial capture. Each must produce the complete pair within the default five-second host deadline. After the final line, observe at least 100 ms of silence with QEMU still running, then stop it from the host and collect the remaining bytes. The observation window and deadline are runner settings. Measuring guest startup latency and physical reset behavior requires separate tests.
2. **Reject false passes.** Compare the full UART byte stream, including its final terminator. A timeout, premature emulator exit, startup error, missing or reversed line, duplicate marker, extra bytes or partial final line fails that launch. Check the verdict with malformed input and a run in which the kernel cannot boot. Preserve diagnostics separately so a line printed by a host tool can never satisfy the UART check. Serial silence after `READY` covers only the observation window. Review the park loop in the candidate when attributing that silence to `WFE`.
3. **Identify and rebuild the candidate.** Record the source revision and any local changes, host, compiler and linker versions and options, target, dependency lock, QEMU version and arguments, ELF SHA-256, and each process's UART, diagnostics and stop reason. Rebuild the same inputs in a second clean build directory and compare ELF SHA-256 values. A difference leaves reproducibility open even if both images boot. A fresh verifier freezes the initial black-box verdict and logs before examining implementation details or earlier team results. Record the evidence separately from this contract. [Evidence template](templates/verification-evidence.en.md)

## Handoff to scheduler work

Carry the identified bootable ELF and the repeatable boot check forward as a regression baseline. Before testing fixed-priority dispatch, define the first runnable thread's saved context and stack, exception vectors, timer/IRQ setup and acknowledgement, and an observation that distinguishes which thread ran. Give priority choice, equal-priority ordering and preemption their own expected traces and failure cases. Task lifecycles, scheduling, timers, isolation and IPC remain open M1 deliverables. Native applications, Linux compatibility, SMP, physical-board timing and safety evidence are outside this boot verdict. [Scheduler design](architecture.en.md) · [M1 milestone](product-plan.en.md)
