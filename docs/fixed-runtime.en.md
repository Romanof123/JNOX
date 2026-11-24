# Single-core fixed-priority runtime

[한국어](fixed-runtime.md) · English

CHG-032 / IMP-030 defines the profile for issue #69. EVD-030 is the target evidence record. The cases below are acceptance criteria.

## Build and run on QEMU

From the repository root on Linux or WSL, use the pinned Rust toolchain, the AArch64 bare-metal target and `qemu-system-aarch64`. These commands use a separate release directory for the selected application.

```sh
app=fixed-fifo
out="$PWD/out/$app"
(cd kernel && JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out" cargo +1.95.0 build --release --locked -j1)
python3 tools/qemu_boot.py --profile "$app" --runs 1 --kernel "$out/aarch64-unknown-none/release/jnox-kernel"
```

Set `app=fixed-rr` for round robin. The other tables are `fixed-empty` and `fixed-delayed`. [Linked task tables](../kernel/src/fixed_apps.rs) · [Scheduler interface](../kernel/src/fixed_runtime.rs)

## Build and application interface

Select `JNOX_PROFILE=fixed` with no benchmark features and select the linked table using `JNOX_APP=fixed-fifo` or `JNOX_APP=fixed-rr`. Two additional configurations are `fixed-empty` and `fixed-delayed`. An unset selector retains the historical default entry and the existing Cargo `--all-features` behavior. Reject conflicting explicit selectors during configuration. The fixed kernel links an application configuration at build time. Changing its task entries, order, priorities, initial delays or peer policy changes the resulting image. The scheduler never expects particular task names, boot order, marker counts or shutdown times.

The Rust boundary exposes `Task { entry: fn(), priority: u8, delay: u64 }`, `Mode::Fifo`, `Mode::RoundRobin(u64)`, and the task calls `yield_now()`, `sleep_ticks(u64) -> bool` and `exit() -> !`. The application supplies an ordered table of zero through four distinct slots. Each slot contains a linked Rust task entry, a fixed priority (larger unsigned byte wins), and an initial delay in physical counter ticks. The array index identifies the slot throughout its lifetime. Two slots may call the same function on separate stacks. The configuration selects FIFO or round robin with one positive quantum in counter ticks. A task starts once and may return normally through a trampoline that exits it. The application uses three operations.

| Operation | AArch64 synchronous ABI | Effect |
| --- | --- | --- |
| Yield | SVC #0 | Move to the back of the ready peers with a fresh slice. |
| Sleep for ticks | SVC #2, x0 = unsigned tick count | Zero yields. A positive duration blocks until the checked absolute deadline. On resumption x0 = 1. A count above i64::MAX or absolute addition overflow returns x0 = 0 without a voluntary scheduling change. |
| Exit | SVC #1 | Permanently remove this slot. Execution never returns to the caller. |

The application-facing module exposes configuration and task operations. Platform code owns vectors, frame fabrication/restoration, GIC, counter, timer and idle. All entries are trusted, linked EL1 code in one address space. The ABI preserves task state across interrupts and services except for the declared result. User-mode tasks, access control and safety-domain isolation belong to future work. The runtime uses static slots, one private idle context, bounded queue storage and separate aligned stacks. Record the ELF footprint, static stack and slot reservations and linked entry addresses for each configuration. Trusted entries must stay within their reserved stacks and preserve platform-owned timers and interrupt state. It provides no heap allocation, dynamic creation, IPC or blocking primitive other than timed sleep.

## Boot, dispatch and time

Start on one core with IRQs masked. Validate capacity, entries, mode, nonzero RR quantum, usable counter frequency, relative delays and quantum within i64::MAX ticks, checked absolute wake deadlines, a representable initial contested RR compare, and distinct valid stack/frame storage before releasing any task. Use one boot counter sample for all initial delays. Initialize all frames, queue state, vectors, interrupt controller and timer before the first restored context can receive an IRQ. A zero delay makes the slot ready in table order. If every slot is delayed, start private idle with the earliest deadline armed. An empty image starts quiet idle immediately. Fatal configuration or unexpected exception errors mask further interrupts, disable the timer, emit a bounded diagnostic if UART is available and park. Never start a partially validated application.

1. Take a monotonic physical-counter sample at each scheduling boundary. Charge only the actual runner for elapsed ticks since the previous decision. RR residual saturates at zero even when the runner had no peer. FIFO has no slice. Execution assumes the counter does not wrap within the boot session and absolute compare deadlines remain representable.
2. Admit every due sleeper in increasing absolute deadline order, breaking ties by slot index. Append each to its ready priority with a fresh slice. Then apply the caller's voluntary operation if any. A positive sleep uses the current sample as its base. Exit, sleep or yield removes the old runner from contention in that role, so it cannot also be rotated on the same boundary.
3. If the runner remains eligible, an exhausted RR runner with a ready equal peer goes to the tail with a fresh slice. Otherwise strict higher-priority preemption keeps the interrupted runner at the head of its priority and preserves its residual. FIFO peers run until they yield, block or exit, subject to higher-priority interrupts. Pick the highest ready priority and its first slot. Idle participates only when there is no runnable application.
4. Program one absolute compare for the earliest pending wake or the running RR task's residual expiry if an equal peer is ready. With no equal peer there is no quantum interrupt. If an active contested RR compare would overflow, deadline calculation returns CompareOverflow and the adapter enters fatal stop. A pending expiration must never silently disable the timer. A later peer encounters the actual remaining slice, including zero. A delayed handler releases all overdue slots and dispatches once, without inventing CPU service or rotations for absent peers. A deadline passing during programming must remain pending or be reconsidered immediately. Acknowledge and complete the timer IRQ through the controller. Disable the timer when no deadline remains. After all exits, private idle waits quietly without periodic ticks, task resurrection or repeated UART output.

## EVD-030 acceptance and raw records

| Case | Falsifiable observation |
| --- | --- |
| Compilation and startup | Two separately built ELFs link genuinely different application tables. Record entries, mode, priority and delay for both. Each guest starts the highest ready task or idle as its own configuration dictates. Include the unset/default and historical all-feature build checks. |
| FIFO and strict priority | Peers enter in ready order. Yield moves the caller behind its peer. An interrupt for a delayed higher-priority entry takes the CPU from a still-executing lower entry. Higher exit restores the interrupted entry before its waiting peer. |
| RR residual and ties | A higher-priority release interrupts an unfinished slice. After resumption the original residual governs rotation. Check equal-peer arrival during uncontested execution, simultaneous release/expiry, multiple late wakes and a late handler spanning several quanta. Compare counter arithmetic and actual slot transitions. |
| Task lifecycle | Observe positive sleep, zero sleep, normal return and explicit exit. Check overflow refusal and startup rejection of invalid capacity or zero RR quantum. Exercise no ready entries at boot, no slots, and quiet idle after the final exit. Verify stable frame/stack identity through preemption. |

Preserve candidate revision and dirty diff, exact build commands and selectors, application tables, tool versions, ELF digests and symbols, emulator configuration, counter frequency, raw UART and same-run exception trace. Any application markers come from the selected entries. The generic runtime emits no test verdict. Retain enough event information to reconstruct cause, sample, outgoing/incoming slot or idle, wake deadlines, RR residual, next compare or disabled state, and interrupted/returned PC. Values may be reconstructed from linked code, instrumented records or debugger state with their source identified. Record observation length for terminal idle, host test inputs and expected/actual values for edge cases, and distinguish host-only cases from actual guest transitions. A missing raw artifact, ambiguous PC, trace mismatch or failed control leaves the affected claim open. Retain failed runs with the reason and identify any rerun separately. Single-core emulation evidence establishes neither physical-board timing nor ASIL qualification.

### Fixture and capture mapping

The FIFO fixture orders slots A, B, HIGH. The RR fixture orders A, HIGH, B. Both delay HIGH by Q/2, where Q = 12,500,000 ticks. FIFO A yields to B. B calls sleep(0) with A ready, so B remains ready while HIGH preempts A. HIGH returns, A resumes ahead of B and returns, then B sleeps for 2Q. On wake B checks that sleep(u64::MAX) returns false and explicitly exits. RR uses quantum Q and HIGH consumes at least Q/4 counter ticks. The exported `fixed_app_observations` contains eight little-endian u64 words in this order: SP(A), SP(B), SP(HIGH), progress(A), progress(B), A observed by HIGH, A observed at B entry, B observed near A completion. FIFO requires progress(A) > A observed by HIGH > 0; the other three work measurements are zero. RR requires 0 < A observed by HIGH < A observed at B entry < progress(A), and 0 < B observed near A completion < progress(B).

For each ELF use `python3 tools/qemu_boot.py --kernel "$elf" --profile fixed-fifo` or `fixed-rr`. On Linux, set `FIXED_RUNTIME_CAPTURE_DIR` to a fresh absolute destination and add `--qemu ./tools/fixed_runtime_capture.py` to retain UART, interrupt trace, application observations and CPU registers. The capture requires `aarch64-linux-gnu-nm`. Map the stopped PC into the linked `fixed_runtime_idle` range after the quiet window and map preemption/return PCs to each actual entry.

| Linked fixture | UART suffixes in order (each line starts with FIXED and the uppercase fixture name) | Additional witness |
| --- | --- | --- |
| FIFO | START, A START, B YIELD, A RESUME, HIGH, A DONE, B SLEEP, B DONE | B issues both zero and positive sleep, rejects the out-of-range sleep and exits. |
| RR | START, A START, HIGH, B START, A DONE, B DONE | Two busy peers execute across the high-priority interruption and timer rotation. |
| EMPTY | START | No application slots. The first context is idle, timer disabled. |
| DELAYED | START, TASK | One FIFO slot at priority 1, delay Q/2. The first context is idle with a compare armed. Its timer IRQ dispatches slot 0, which explicitly exits. |

Build the ordinary images with tracing unset and observe their guest behavior independently. For diagnostic fixed images, opt in at build time with `JNOX_FIXED_TRACE=1`. Keep separate ELF digests, build settings and captures for the two classes. The optional recorder stores at most 64 chronological post-event snapshots in static memory. After the last available row, keep existing rows intact and indicate dropped events. Inspect count and overflow before reconstructing dispatch. Each row describes a generic scheduler event and contains no test-specific slot or expected sequence. Compare real event counts, programmed absolute deadlines and timer readbacks against service reconstructed from adjacent samples and the actual outgoing slot. Timing observed in a diagnostic build does not bound product latency.

#### Binary trace schema

The exported `fixed_runtime_trace` has C layout, entirely little-endian u64 words. The header is [count, overflow, frequency, row_words]. Count is the number of stored rows (0–64), overflow counts subsequent dropped rows, frequency is in Hz and row_words is 19. The header is followed by space for 64 rows. Read only the first count rows. `trace.bin` is present for instrumented images; `launch.json` records the linked symbol address and size. Use the matching `--profile fixed-empty` or `--profile fixed-delayed` to capture those configurations.

| Word indexes (zero-based) | Values in storage order |
| --- | --- |
| 0–4 | Event kind (0 boot, 1 timer IRQ, 2 SVC), source (raw INTID or SVC immediate, u64::MAX for boot), counter sample, previous owner, selected owner. Idle owner is 4; boot previous owner is u64::MAX. |
| 5–10 | Incoming exception frame address, saved ELR, interrupted SP, selected frame address, selected ELR, selected SP. SP is the frame address plus frame size. An absent frame has three zero fields. |
| 11–14 | Sleep argument, sleep result (1 accepted, 0 refused), next compare valid flag, intended absolute compare. Both sleep fields are zero for other events. An invalid compare has value zero. |
| 15–18 | Post-programming CNTP_CTL, post-programming CNTP_CVAL, pre-handler CNTP_CTL for IRQ, cumulative service of previous owner after charging this event. Pre-IRQ control and previous-owner service each use u64::MAX when unavailable. CNTP_CVAL may be stale when disabled; inspect CTL and compare validity. |

For chronological rows i and i+1, row i's selected owner runs during the counter interval ending at row i+1. Check that this owner is the next row's previous owner, accumulate sample deltas per app slot and compare against word 18 when available. Use ELR/SP and the corresponding ELF to distinguish entry, interrupted code, resumption and idle.

The empty application keeps its eight observation words at zero. The delayed application records slot 0’s nonzero aligned stack pointer in the first word and leaves the other seven words zero.

First independent results for ordinary and instrumented images and their scope are recorded in [EVD-030](evidence/evd-030.en.md)

[Simultaneous and late timer events](fixed-runtime-edges.en.md) · [#72](https://github.com/Romanof123/JNOX/issues/72)
