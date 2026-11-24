# CHG-072 · Ordinary mutex timer capture

[한국어](ordinary-mutex-anchor.md) · [Approved plan](ordinary-mutex-return-plan.en.md) · [Original cohort](evidence/ordinary-mutex-first/README.md)

The source provides three optional images for each of `ordinary-pi` and `ordinary-ceiling`. Use `JNOX_BOARD=virt JNOX_PROFILE=mutex JNOX_APP=<app>`, the pinned toolchain and the original release build. Set `JNOX_MUTEX_ANCHOR` explicitly for each new cohort. Leaving it unset selects the original Rust implementation and RAW 1. Each selected image uses one PPI30 compare at its own sample plus exactly CNTFRQ/10000. The protected worker, 256 chunks, ceiling midpoint yield, lock policy and work eligibility use the original code.

| Selector | Mode | Sample and instrumentation |
| --- | ---: | --- |
| `rust-prepared` | 1 | Prevalidate the interval at boot and reserve the final-acquire witness before sampling at the original Rust scheduling point. Read CVAL and CTL following ISB. The first IRQ or terminal output correlates the R ledger. |
| `rust-stages` | 2 | Same Rust anchor with ordered write-end, readback-end, and selected restore-entry samples. The first sample follows the timer writes and ISB, just before reading CVAL/CTL. The second follows both readbacks and ISB. The third follows the switch to the selected frame and the instrumentation guard. |
| `selected-return` | 3 | Reserve the successful LOW acquire in Rust. Claim that token at the end of the selected frame restore, with exceptions masked, original task SP restored, registers loaded and CLREX complete. Check the generation, R ordinal, frame, live ELR, SPSR, TPIDR and x0 before arming. Sample again just before restoring temporary registers and executing ERET. |

The bounded record occupies 160 bytes plus the existing ledgers. Its generation is the successful decision's one-based E index. Its designated restore is R[generation+1] in one-based notation, accounting for the initial LOW return. The return hook uses x9–x14 and restores them from their intact saved slots. It allocates no spills and touches no SIMD register. It preserves handler NZCV and the selected SPSR. The failure path masks exceptions, disables CNTP and uses the boot stack to print. Count linked instructions in the fresh frozen ELF. In source the final counter read is followed by one store, NZCV restoration, frame-relative scratch reloads, SP restoration and a branch to ERET. The capture cannot resolve the ERET cycle.

## Retained linked reference

These PCs come from the original PI and ceiling disassembly for source `47ea890`. The PI plain capture's arm-to-R interval was 6,533 counter ticks. That instrumented interval combines the Rust tail and most of the restore.

| Instruction | PI | Ceiling |
| --- | --- | --- |
| CNTPCT sample | 0x40202780 | 0x4020287c |
| CVAL and CTL writes | 0x402027a8, 0x402027ac | 0x402028a4, 0x402028a8 |
| CVAL and CTL readbacks | 0x402027b4, 0x402027b8 | 0x402028b0, 0x402028b4 |
| Success epilogue through RET | 0x402026e4–0x40202700 | 0x402027b4–0x402027d0 |
| Restore entry, R counter, ERET | 0x402019ac, 0x40201a54, 0x40201aa4 | 0x402019ac, 0x40201a54, 0x40201aa4 |

## RAW 2 and replay

Preserve the complete UART byte file. RAW 2 uses the RAW 1 H/T/S/E/R/C/B/HIGH/A/M/K widths, ordering and units. Insert exactly one W and one P row after T. Every value is a sixteen-digit lowercase hexadecimal u64. The existing R sample still precedes final GPR restoration. T reports real arm/start, requested compare and CVAL/CTL readback even if the validated record later fails. A missing stage has its validity bit clear. An elapsed flag with no sample is NA (all ones).

| Row | Fields in wire order |
| --- | --- |
| W (13) | Witness version 1, mode, phase, valid bits, generation, restore ordinal, selected frame, ELR, SPSR, TPIDR_EL1, x0, lock ID, prevalidated interval |
| P (6) | Write-end, readback-end, restore-entry, correlated existing R sample, final pre-ERET sample, elapsed-at-final flag |
| Phase | 0 empty, 1 prepared, 2 claimed, 3 timer programmed and accepted, 4 linked to the recorded restore |
| Valid bits | 0x001 prepared, 0x002 start, 0x004 target, 0x008 hardware readbacks, 0x010 write-end, 0x020 readback-end, 0x040 restore-entry, 0x080 final pre-ERET, 0x100 correlated R |
| Additional terminal reasons | 19 token identity, ordinal or reuse, 20 timer readback, 21 counter addition overflow, 22 return correlation |

A full accepted record requires valid bits 0x10f, 0x17f or 0x18f respectively. ISTATUS at the arm readback and target already elapsed at the final sample are observable outcomes. Each token can program the timer once. Preserve the original failure and its RAW 1 denominator.

Freeze separate PI/plain, PI/traced, ceiling/plain and ceiling/traced positions for **each** selector, using a fresh Linux-native directory, one CPU, Cortex-A72, virt/GICv2/TCG, 128 MiB, default clock, plain/trace pair sharing its ELF, 15 s wall allowance and the original host stop on the complete footer. Preserve every first launch including early exits, timeout and partial output, raw UART/stderr, argv, elapsed time, PID/status, ELF and source digest, linked disassembly, and external exception log where requested.

Run `python3 tools/mutex_anchor_replay.py <uart> --app ordinary-pi --anchor selected-return` with the matching inputs. Exit 0 checks the anchor, completion data and IRQ markers. Exit 1 retains a syntactically complete terminal failure. Exit 2 reports malformed or inconsistent evidence. The decoder bounds input to 96 KiB, 32 decisions, 64 restores and 256 chunks. Independent qualification also requires lock/ready/handoff replay, saved PC inside actual input computation in the same linked image, stacks, real data, exits, and optional external SVC/IRQ/ERET matching as specified in the original protocol. Inspect linked default PI, ceiling and legacy mutex images against the retained ELF, PT_LOAD and text sections. Variant timings, linked register preservation, default binary equivalence and physical measurements await verification. [Original replay criteria](ordinary-mutex-runtime.en.md) · [Offline decoder](../tools/mutex_anchor_replay.py)

[First default PI preflight: identical instructions, changed loadable metadata, zero guest launches](evidence/ordinary-mutex-anchor-preflight/README.md)
