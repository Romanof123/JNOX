# Pi 4 MG2 migration · Causal capture plan

**The six original positions remain 0/6. The SGI write, its acceptance inside the GIC model and M’s final arithmetic cannot be ordered from the retained traces alone.** This plan preserves the existing kernel workload and proposes two diagnostic stages to identify an actionable defect. [#47](https://github.com/Romanof123/JNOX/issues/47) · [MG2 contract](pi4-smp-migration.en.md) · [First results](../evidence/pi4-migration-mg2-first.en.md) · [한국어](pi4-mg2-causal-capture.md)

## Retained evidence

At main `0e21061e841261ea49da14e1ced5327320759e73`, the migration, workload, BSP, journal and capture files match their versions at the tested source `559449c673ac55a0a2bf34f3e8ae680d99e72f09`. An offline review checked the archive’s 82 manifest entries, all six public session digests, and the per-CPU exception streams against each raw image. The table uses half-open intervals. Each row describes both first positions for that regime. [Reproducible audit](../../tools/pi4_mg2_retained_sites.py) · [Per-image addresses, hashes and trace records](../evidence/pi4-mg2-causal-sites.json)

| Regime | Strict M loop | CPU0 terminal SVC ELR | CPU1 opening SVC ELR | CPU1 idle ERET PC |
|---|---|---|---|---|
| Global | `[0x81f00,0x81f24)` | `0x81f9c` | `0x82098` | `0x809a8` |
| Partitioned | `[0x81f04,0x81f28)` | `0x81fa0` | `0x8209c` | `0x809ac` |
| Clustered | `[0x81fb0,0x81fd4)` | `0x8204c` | `0x82148` | `0x809ac` |

CPU0’s SVC saved an IRQ-enabled PSTATE and the expected M sum on the failure branch. CPU1 acquired the index-16 milestone, entered its SVC, retired P1’s lease, issued the directed SGI and returned to its private idle frame. The CPU0 verdict occurs inside the same scheduler lock and parks there. That orders P1’s completed SGIR write before the verdict. CPU0 can already have issued its SVC before it acquires that lock. Its saved I bit describes SVC entry only. The published `PEER_READY` occurs before P1’s actual first ERET. Neither a delayed peer activation, a wrong GIC configuration nor delayed interrupt delivery has been established as the cause. [Workload](../../kernel/src/pi4_migration_tasks.rs) · [Opening, retire and terminal paths](../../kernel/src/pi4_migration.rs) · [GIC send and acknowledge](../../kernel/src/pi4_smp_bsp.rs)

## Stage 1 · Read the committed evidence after termination

Use the archived stock QEMU digest, the three exact raw images, four model CPUs with only 0 and 1 online, and the original MTTCG flags, observer, 30-second ceiling and one-second quiet interval. Predeclare two diagnostic positions per regime, in global, partitioned, clustered order. Add a local QMP capture channel. After terminal quiet and before process termination, stop the already-terminal guest and save the linked portions of RAM needed for page-safe access to both local journals, counts, frame storage, private stacks and scheduler state. Record terminal and per-CPU parked PCs, actual RAM ranges and symbol sizes, launch arguments, timestamps, raw digests, QEMU thread identities and every incomplete capture. A timeout dump is a partial observation.

Decode only fully release-committed local rows. Preserve lost counts and both unsealed prefixes on the 0x1d branch. Join the original local lease IDs, M’s sparse checkpoints, P1’s count/digest/finish/retirement, its SGIR word and the CPU0 terminal SVC frame. Validate the expected index and triangular sum, M’s 4,096-checkpoint digest where present, and local entry/return stamps against external PCs. The 0x1d branch skips M’s kind-13 row and final UART journal export. A saved M frame may still contain the SVC arguments. A stopped RAM snapshot does not give a timestamp for a pending transition or an entry that never happened.

## Stage 2 · Distinguish activation, send and delivery

Freeze a diagnostic observer against the pinned QEMU revision and archived guest images. Retain the observer source, model patch where needed, compiler and binary digests. Use bounded single-writer rings and select the existing **sparse** checkpoint call. The reader decodes its task, lease, index and sum. A global trace hook on every arithmetic instruction or checkpoint would strongly alter M’s scheduling. Use the linked sparse sites, the unchanged external exception observer and a bounded failure snapshot. Preserve all ordinary guest code, the single milestone, both directed interrupts, task masks, private stacks, accumulators and timers.

| Producer | Causal observations |
|---|---|
| CPU0 | Actual ERET into M, sparse epoch-0 checkpoints at 16, 32 and 65,536 where reached, first-milestone publication, terminal SVC entry or actual IRQ entry, observed PSTATE, saved ELR/X21, first actual resumed ERET. The final sparse row follows the final add. |
| CPU1 | Actual ERET into P1, positive milestone branch after its acquire, SVC entry, scheduler-lock acquisition, validated opening lease retirement, SGIR store operand/address and subsequent barrier completion, ERET to idle. |
| GIC and target model | Record the real decoded SGIR transaction, source CPU, Group/ID and destination mask, per-target pending-state change, target IRQ-output assertion and CPU0 interrupt acceptance or mask/exception boundary as exposed by the model. Retain CPU0’s **existing** IAR read and EOI write if reached. Extra IAR reads would acknowledge the interrupt. |

Use each producer’s sequence and local counter or host monotonic sample for intervals within that producer. Bind CPU0’s index-16 release to P1’s successful acquire, then join the actual store, GIC pending transition and CPU0 acceptance by run, opening generation, source, destination, Group and SGI ID. Handle the reverse directed SGI separately. At the existing sparse and terminal boundaries, a host observer can release-publish a completed marker in a per-producer monotone slot and acquire-sample the peer’s previously completed marker. Record both sampled sequence and originating event. Only positive reads justify additional cross-thread edges. Record entry and completion separately for operations with side effects. File order, host thread-ID order, numerically equal counter rates, a single cross-thread timestamp or an absent peer sample cannot order the guest events. Keep overlapping brackets ambiguous.

For example, a completed SGIR sampled at M’s positive index-32 checkpoint proves that later arithmetic remained. If the GIC write sees the already committed final M checkpoint, M had completed its arithmetic before that modeled transaction. When the two only bracket the final interval, report its endpoints and leave the order open. Record local intervals for P1 entry through acquire, SVC entry through lock/retirement and SGIR, and CPU0 activation through checkpoints and IRQ/SVC. A pending IRQ in a terminal snapshot alone does not establish a broken controller. A demonstrated misprogrammed target or barrier, or a delayed P1 launch before its actual first ERET, points to a specific path to repair. Determine whether that path caused the missed delivery from the trace before changing the kernel.

Freeze diagnostic positions independently from the original six and retain every first attempt of each stage. Report observer overhead and the exact first unmatched event. Only the original uninstrumented qualification contract can credit a release attempt. A future corrected candidate must receive its own six first launches with its frozen ELF/raw/map/loop limits and ordinary CPU-indexed exception history. Require the exact linked loop PC, matching restored frame and ERET, subsequent owner-bound work and all four activation summaries. Physical Pi behavior and cross-core latency bounds remain open.

## Stage 1 collector

From a clean Linux checkout, use the archived QEMU 11.1.3 executable and Python with pyelftools. `freeze` only hashes the model executable. It extracts the three exact archived inputs from the verified first-run ZIP, freezes all six positions, records the writable PT_LOAD and symbol extents from each ELF and prints the trusted cohort digest.

```sh
python3 tools/pi4_mg2_terminal.py freeze /path/to/new-cohort /path/to/qemu-system-aarch64
python3 tools/pi4_mg2_terminal.py run /path/to/new-cohort COHORT_SHA256 1
python3 tools/pi4_mg2_terminal.py replay /path/to/new-cohort COHORT_SHA256 1 SESSION_SHA256
```

Use a native Linux evidence path. Run positions 1–6 in order with one guest at a time. Record the session digest printed by each launch. Every attempted position uses a new directory. The capture return code describes the observer. The replay labels the guest outcome separately. Preserve a failed or unobserved position before proceeding. The original six are stored separately in the existing archive.

The host supplies a private QMP socket without waiting for a client. Its first QMP command follows the original one-second quiet interval after the UART terminal. A nonterminal deadline or early process exit preserves the partial serial/exception evidence without halting an active M for a snapshot. After a confirmed stop the collector saves exactly the linked writable segment and records the four model CPUs’ PCs and Linux thread IDs. The partial journal layout was read from the archived event and seal instructions. Every 72-byte row has eight data words followed by its tag, with 96 rows in each 6,944-byte local journal. Only the committed prefix is decoded. The raw bytes and frozen symbol extents also preserve scheduler state, seven stacks, return stamps and thread-local ownership observations. A stopped PC is a sample taken after the quiet interval.

The decoder reports a verified early M return only when the matching external CPU0 SVC, private frame and independently recomputed arithmetic all agree. A sealed completion additionally requires matching RAM/UART rows and the model’s independent exception history. Lost records, unsealed prefixes, local intervals and unavailable joins are retained. The collector neither measures the GIC pending edge nor dates the SGIR against the last M addition. Stage 2 and the original migration qualification remain distinct.
