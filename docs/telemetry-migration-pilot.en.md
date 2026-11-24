# AArch64 application migration pilot · Telemetry Monitor

[한국어](telemetry-migration-pilot.md) · CHG-084 · Planned acceptance

The product goal is to replace the OS beneath an identified safety application while preserving its required behavior, device access and fault response. The first engineering pilot is a headless file-to-summary process using the project's own C source, `examples/portability/telemetry_monitor.c`. Compile that same file separately with each OS's SDK and libc startup. A commercial application source, originating-system SDK, runnable donor image and device are currently unavailable. Record a host baseline and leave those comparisons open until the integrator supplies them. [Product plan](product-plan.en.md) · [Platform checklist](platform-checklist.en.md)

## Process contract

Pass exactly one pathname. Read its bytes with `open` (`O_RDONLY`), `read` and `close`; write the result via `write`. Accept one or more ASCII decimal digits per record followed by LF, including the final record. Values range from 0 through 65535 inclusive; leading zeros are valid. Reject signs, spaces, blank lines, CR, NUL, other non-digit bytes and an unterminated final record. Parse across short reads and accumulate a checked 64-bit unsigned count and sum. Retry interrupted open/read/write and advance by each successful short transfer. Call close once per opened input. Complete input processing and close before publishing the summary.

| Exit | Observation with writable standard streams |
| --- | --- |
| 0 | stdout `samples=N min=L max=H sum=S\n`, minimal unsigned decimal, empty stderr |
| 2 | Wrong argument count. stderr `usage: telemetry_monitor PATH\n` |
| 3 | Zero input bytes. stderr `empty input\n` |
| 4 | Malformed or out-of-range record. stderr `invalid sample\n` |
| 5 | Input open, read or close failure, including a missing path. stderr `input error\n` |
| 6 | Count or sum cannot fit in uint64. stderr `summary overflow\n` |
| 7 | Summary write fails. stderr `output error\n`; earlier stdout bytes may already be visible |

Here `\n` denotes the single byte 0A. Exits 2–6 have empty stdout. The bytes `0\n0007\n65535\n` give `samples=3 min=0 max=65535 sum=65542\n`. Distinguish a zero-byte file (3), one LF (4), `7` without LF (4), `65536\n` (4) and an absent path (5). An arithmetic boundary check can exercise overflow separately; identify what was observed in an actual process.

## Evidence progression

| Gate and owner | Observation required to advance |
| --- | --- |
| 1 · Application and independent verification | Freeze source and input hashes, command line, executable and compiler/sysroot identity per OS. Run the identical corpus, including records split across reads, on a named host. A verifier independently computes expected bytes and captures both streams and the parent-observed exit status for each case. Add the same observations from a licensed originating-system SDK and target when available. Record unsupported cases, failures and the exact platform for each result. |
| 2 · Kernel, IPC, filesystem and SDK | Complete applicable scheduling acceptance in #21 before opening IPC development in #24. Build a separate static AArch64 ELF with public C headers, libc/startup and link map. Load it into an isolated EL0 address space with argv and a permitted input path in the read-only root image. Route descriptors through the user-space namespace/file service over authorized IPC. Capture privilege/entry, read-only open, reads through EOF, output writes, close, exit and parent collection. On failure observe return values and relevant errno (e.g. −1/ENOENT at missing open); compare process outputs with gate 1. Repeated launch must reclaim mappings, descriptors and pending requests. [IPC profiles](ipc-mechanisms.en.md) · [Filesystem plan](filesystem-plan.en.md) |
| 3 · Services, drivers and operations | On the selected device migrate the actual data source to a named resource service. Choose native Rust driver or isolated Linux driver carrier, declare IRQ, MMIO, DMA and reset authority. Exercise service loss, stale handle rejection, bounded completion/reclaim, restart and fresh open with competing load. Retain a bounded timestamped process/IPC/driver trace with lost-event count and measured capture overhead. The broader #8 program separately verifies the native C shell pipeline, native Rust SDK application, selected POSIX APIs and same-ISA prebuilt Linux ELF. [Platform checklist](platform-checklist.en.md) · [POSIX profile](posix-profile.en.md) |
| 4 · Product owner, BSP and functional safety | Approve the chosen board, budget and owner before purchase. Obtain the real application and donor baselines, exact board/revision, input device, actuator, operating conditions and safety manual for product qualification. For the separately selected control path, use HARA to derive the safe state, ASIL, fault-tolerant time interval, monotonic deadlines and allocations. On the specified physical system measure input-to-output response, load/IRQ/DMA interference, loss, lateness, restart and physical safe-state entry through the actuator with an independent timebase. Compare measured maxima and justified bounds with the allocations and margin. Pin the firmware, signed image, toolchain and recovery procedure; record residual risks, independent assessment and item integration approval. [Clock plan](system-time-plan.en.md) · [Safety decision ADR-001](safety-parameters.en.md) |

Record gate status per named OS, configuration and device. The first file pilot covers four C calls and process startup. Replacement of an existing product remains open through the applicable kernel, service, device, operational and physical safety gates. Trace CHG-084 to REQ-014/020 and DES-005 / VER-014/020; #8 retains the complete SDK and API acceptance cases. [Verification plan](verification-plan.en.md)
