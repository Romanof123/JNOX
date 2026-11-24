# M1 single-core fixed-priority preemption bench

[한국어](m1-scheduler.md) · English

The verifier compared UART output and exception addresses from the same boot. [EVD-002 verification results](evidence/evd-002.en.md)

A separate profile exercises two equal-priority threads becoming ready, yielding and exiting. [FIFO acceptance contract](m1-fifo.en.md)

`CHG-002` adds the optional `sched-bench` Cargo feature to boot baseline `d17052a`. `IMP-002` addresses one question. If a timer IRQ makes a high-priority thread (HIGH) ready while a low-priority thread (LOW) keeps using the CPU, does HIGH run to completion before LOW continues the interrupted computation? The default build uses the existing two-line UART contract for boot regression checks. [M1 boot bench](m1-boot.en.md)

| Trace | Behavior under observation |
| --- | --- |
| `REQ-002` → `DES-003` → `IMP-002` · `VER-002` | The timer interrupts LOW. With both threads runnable, the scheduler selects HIGH by fixed priority. |
| `REQ-005` → `DES-003` → `IMP-002` · `VER-005` | Each thread executes on its own stack in the same address space. Finishing HIGH causes a return to LOW's saved context. |

[System requirements](requirements.en.md) · [Scheduler design](architecture.en.md) · [Verification plan](verification-plan.en.md)

## Execution sequence and witnesses

Use QEMU `virt` with TCG, GICv2, one Cortex-A72 CPU and 128 MiB of RAM. Boot the guest kernel ELF directly and capture the first UART. The two kernel threads may be statically allocated and have distinct, fixed priorities. Initially only LOW is runnable. HIGH waits for the timer.

1. **LOW occupies the CPU.** LOW starts on its own stack, emits `SCHED LOW RUN` and enters a non-yielding work region. It keeps computing without calling the scheduler, requesting a wait or invoking HIGH. The IRQ must interrupt execution within this region.
2. **The IRQ makes HIGH ready.** Exception entry preserves the general registers, stack pointer, return PC and processor state needed to resume LOW. The IRQ path identifies the expected timer interrupt through the GIC before emitting `SCHED TIMER IRQ`. It inhibits further timer delivery and signals completion to the GIC. While LOW remains runnable, the scheduler selects HIGH by priority. Exception return enters HIGH's thread context.
3. **HIGH finishes on its own stack.** HIGH emits `SCHED HIGH RUN` from its thread body. Once its work completes, HIGH becomes ineligible to run and the scheduler restores LOW's saved context.
4. **LOW continues its interrupted computation.** LOW resumes with its restored PC and stack. After confirming HIGH completed and LOW retained state from before the interrupt, LOW emits `SCHED LOW RESUMED`. Only the successful sequence may reach `SCHED PASS`.

The independent reviewer traces the markers to these events. Check that the IRQ return address was in LOW's non-yielding region, both threads were runnable at selection, and the priority comparison selected HIGH. Check for disjoint stack ranges, execution of HIGH on its own stack, and restoration of LOW's original stack and execution state. Examples of witnesses include exception-frame and stack-bound checks, a stack local or register value read across the interruption, and control flow that prevents a pass when these checks fail. A direct call from the handler to HIGH or a restart at LOW's entry fails this case even if it produces the expected strings.

## Build and UART verdict

Start from the repository root with the pinned Rust toolchain, the `aarch64-unknown-none` target, Python 3 and `qemu-system-aarch64`. The default unit of execution is one QEMU guest. Repeat cold starts sequentially.

```sh
cd kernel
cargo build --release --locked --features sched-bench -j 1
cd ..
python3 tools/qemu_boot.py --profile scheduler --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

The complete guest UART byte stream for one successful launch consists of the following seven lines, each terminated by LF or CRLF. `READY` marks completion of the existing Rust boot check.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
SCHED HIGH RUN
SCHED LOW RESUMED
SCHED PASS
```

A missing, reordered or duplicate line, extra bytes, a missing final terminator, a host timeout or a premature guest exit during observation fails the run. Retain the raw UART, separate emulator diagnostics, stop reason, source revision, configuration and ELF SHA-256. For negative controls, replay the recorded default boot UART from `EVD-001` and the current trace with the resumption line removed through the scheduler observer on the host. The same observations support multiple checks from a single guest boot. [Evidence template](templates/verification-evidence.en.md)

## Independent verdict and scope

A fresh verifier receives the public contract and identified candidate, preserves the initial black-box verdict and raw logs, and then reviews the implementation and structural witnesses. Acceptance of this case requires both the UART sequence and evidence for the actual context transitions. Record any unverified condition and its reason. [Engineering lifecycle](lifecycle.en.md)

This bench defines partial evidence for `VER-002` and `VER-005` covering one preemption and resumption on one core. FIFO order among equal priorities, the general thread lifecycle, SMP and budget-overrun handling need separate tests. The host deadline bounds the test run. IRQ latency, worst-case response time, physical-board deadlines and product safety approval each require separate evidence.
