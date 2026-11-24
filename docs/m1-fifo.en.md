# M1 single-core FIFO scheduling bench

[한국어](m1-fifo.md) · English

The independent verifier recorded UART, the interrupt and exception returns from the same guest. [EVD-003 execution results](evidence/evd-003.en.md)

`CHG-003` freezes the acceptance case for `IMP-003` against upstream baseline `715b02f`. Two equal-priority threads become ready in one timer interrupt. The bench checks which thread runs first, where a yielding thread re-enters the ready queue and when the interrupted lower-priority thread can resume. [Preemption baseline](m1-scheduler.en.md)

| Trace | Acceptance condition |
| --- | --- |
| `REQ-002` → `DES-003` → `IMP-003` → `VER-002` | The timer preempts LOW. A and B have the same fixed priority above LOW. The scheduler dispatches ready threads at the highest priority, in arrival order within that priority. |
| `REQ-005` → `DES-003` → `IMP-003` → `VER-005` | LOW, A and B execute on separate stacks in one address space. Each peer resumes after its yield, exits once and becomes ineligible. LOW then continues its interrupted work. |

[Requirements](requirements.en.md) · [Scheduler design](architecture.en.md) · [Verification plan](verification-plan.en.md)

## Guest sequence

1. LOW starts alone and enters its non-yielding work region. A real timer IRQ interrupts that region. The handler identifies the timer through GICv2, makes A ready and then B ready before dispatch, and completes the interrupt. A has a **higher numeric thread ID** than B so ascending-ID selection would pick B first.
2. A runs first and yields to the tail behind B. B runs and yields to the tail behind A. Both remain runnable across their yields. A resumes after its yield and exits. B resumes after its yield and exits. LOW remains runnable throughout.
3. With both peers finished, LOW resumes its interrupted computation and checks its saved state before reporting success.

## Build and verdict

Build the feature once and boot the resulting ELF once in QEMU `virt` with GICv2, TCG, one Cortex-A72 CPU and 128 MiB RAM. From the repository root, run these commands with the pinned Rust toolchain, the `aarch64-unknown-none` target, Python 3 and `qemu-system-aarch64`.

```sh
cd kernel
cargo build --release --locked --features fifo-bench -j 1
cd ..
python3 tools/qemu_boot.py --profile fifo --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

The complete UART stream must contain exactly these ten lines, each terminated by LF or CRLF. The guest must stay alive through the observer's quiet window until the host stops it.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
FIFO A FIRST
FIFO B FIRST
FIFO A RESUMED
FIFO B RESUMED
SCHED LOW RESUMED
SCHED PASS
```

The independent verifier retains the first UART verdict, raw bytes, emulator diagnostics, stop reason, candidate revision and ELF SHA-256. Use host-only replay for negative controls, including swapped first-dispatch lines and a missing resumption line. Missing, duplicate, reordered or extra bytes, an incomplete last line, timeout and early exit fail the observer. [Evidence template](templates/verification-evidence.en.md)

Using the same ELF and source, check the IRQ return PC against LOW's work region, the priority and ready-queue operations, the three stack ranges and the saved return locations across both yields. Verify that exit removes each peer from selection and that LOW checks state retained across preemption. Acceptance requires both the recorded sequence and these structural checks. This case contributes partial single-core evidence to `VER-002` and `VER-005`. SMP, time slicing and physical timing bounds require separate evidence.
