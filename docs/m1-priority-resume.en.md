# Fixed-priority resume contract

CHG-029 / IMP-027 / planned EVD-027 · POL-002 · #63, parent #21 · REQ-002/005/023, DES-003/012, VER-002/005/023.

One AArch64 virt CPU runs three epochs on separate stacks. A starts first. CNTP PPI 30 admits same-priority B after one quarter quantum of A service and HIGH after a further quarter. Both FIFO variants retain A on B arrival. Preemptive FIFO saves A ahead of B on HIGH arrival. HIGH exits, A resumes its exact instruction, yields to B, and B exits to A's post-yield instruction. Deferred FIFO returns to A on both arrivals. A yields, HIGH exits to B, then B exits to A. In preemptive RR, HIGH exits to interrupted A. A expires after its residual service, B exits and A completes. Each epoch ends with an empty queue.

The quantum is floor(CNTFRQ/10) physical counter ticks. Charge each dispatch from the scheduler's final counter sample before restoring a frame to the first sample in the next exception callback, including vector overhead. Exclude scheduler decision time. On each release subtract measured service, retain the residual across HIGH, and program exactly that residual on resume. At expiry accept at most one eighth quantum of interrupt lateness. The second release uses the same delay in both FIFO variants. Positive progress, task-owned stack words, register witnesses and saved/selected ELRs accompany dispatch decisions. Independent first-boot review must replay queue, service and exception returns and repeat the existing FIFO/RR profiles. Physical timing and product acceptance remain under #21.

```sh
PRIORITY_RESUME_CAPTURE_DIR="$capture_dir" python3 tools/qemu_boot.py --profile priority-resume --qemu ./tools/priority_resume_capture.py --kernel "$kernel_elf"
```

## Independent replay of the first build

Use the ELF from [PR #67’s first implementation commit](https://github.com/Romanof123/JNOX/commit/93c64c853c3d1bf3f6286da59d83d7dc8394f7b9) and retain its hash, symbols and disassembly with the first boot's `audit.bin`, `int.trace` and UART. Each boot has its own physical counter origin. All timestamps are raw CNTPCT_EL0 values since the emulated system counter's origin. Differences and budgets use ticks at the header frequency (ticks/second).

At ELF symbol `priority_resume_witness`, read unsigned little-endian 64-bit words. Header words 0–2 are committed row count, CNTFRQ_EL0, quantum. Capacity is 32 rows. Decode only the committed prefix, with row i starting at byte 24 + 144 × i. A complete run has 21 rows, seven per epoch. The 18 columns below are zero-indexed.

| Word | Field | Meaning |
| --- | --- | --- |
| 0 | `epoch` | Epoch key |
| 1 | `event` | Decision event key |
| 2 | `from` | Outgoing task |
| 3 | `to` | Selected task |
| 4 | `queue` | Ready queue after selection |
| 5 | `begin` | Start counter of outgoing grant |
| 6 | `end` | Exception-entry callback counter of outgoing grant |
| 7 | `due` | Absolute deadline programmed for outgoing grant |
| 8 | `budget_before` | Outgoing RR ticks available at grant start |
| 9 | `budget_after` | Outgoing RR ticks after charging this interval, before expiry resets the quantum |
| 10 | `next_begin` | Selected grant start counter |
| 11 | `next_due` | Absolute deadline programmed for selected grant |
| 12 | `next_budget` | Selected task’s available RR ticks |
| 13 | `save_pc` | ELR saved for outgoing task |
| 14 | `return_pc` | ELR in selected frame |
| 15 | `save_frame` | Address of outgoing exception frame |
| 16 | `return_frame` | Address of selected frame |
| 17 | `progress` | Outgoing worker’s saved X20 loop-progress value |

Epochs 0, 1, 2 select preemptive FIFO, deferred FIFO and preemptive RR. Events 0–5 denote epoch start, B arrival IRQ, HIGH arrival IRQ, yield SVC #2, exit SVC #1 and RR expiry IRQ. Task IDs are HIGH=0, B=1, A=2 and NONE=3. Decode the ready queue from the least significant nibble first, with each occupied nibble equal to task ID + 1. Zero ends the queue. This is insertion order across priorities. The selected task is outside the queue. Zero due means timer disabled. FIFO budgets are zero. Boot has from=NONE and zero outgoing fields. Final exit has to=NONE and zero selected fields.

1. Start each epoch with A. Append arrivals and yielding or expired tasks at the tail. On strict higher-priority preemption, put the interrupted task at the front. Retain the runner on an equal-priority arrival and defer both FIFO arrivals in epoch 1. On a selection boundary take the first highest-priority task. Compare every computed from, to and queue, including empty final queues.
2. Match consecutive grants within each epoch by task ID and carry next_begin, next_due and next_budget into begin, due and budget_before. Verify the header quantum and both quarter-quantum release deadlines. Require end > begin and an IRQ delivery in [due, due + floor(quantum/8)]. In RR independently calculate max(0, budget_before − (end − begin)). Carry A’s remainder unchanged through HIGH’s exit. Check next_due − next_begin equals this residual on A’s resumption, and that the resulting expiry reaches zero. Compare the next available budget after expiry with a fresh quantum.
3. Align each IRQ/SVC row to its CPU0 exception in trace order. IRQ saved ELR lies in the three-instruction A work loop. For SVC, locate the linked SVC instruction, check its immediate and compare the saved ELR to its address + 4. Compare each actual exception-return PC to return_pc. Initial boot uses a synthetic entry frame. A terminal exit with to=NONE has no return to that epoch. At a transition, the next epoch’s boot row describes the ERET from the preceding terminal SVC. The last terminal SVC parks.
4. For retained A compare both frame addresses and PCs within the same row. Across switches pair each suspended frame address and saved PC with its later selection, including post-yield restoration. Check distinct task stacks using linked frame size and stack bounds. Compare outgoing X20 with prior observations of that worker, require positive progress, and check B/HIGH exit with 4097. Record the existing FIFO and RR profile regression results separately.

The launcher runs a local Python capture script with its current Python interpreter, including when the checkout uses Windows line endings.
