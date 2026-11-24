# Ordinary Rust mutex workload and raw capture

CHG-067 uses one cold virt/GICv2 Cortex-A72 CPU, 128 MiB, the default clock and physical PPI30. From kernel/ build with Rust 1.95.0, cargo build --locked --release -j1, JNOX_BOARD=virt and JNOX_PROFILE=mutex. Set JNOX_APP to ordinary-pi or ordinary-ceiling. External QEMU exception tracing uses the same ELF as the plain run. The selector rejects a missing/unknown app, Pi 4, a competing benchmark, fixed trace or a control case. The existing mutex-bench remains the assembly inversion, timeout and ceiling fixture.

IDs 0/1/2/3 denote LOW/MEDIUM/HIGH/idle, base priorities 2/5/9/0. Resource 0 is X and 1 is Y. PI exposes only X. Ceiling X admits LOW/MEDIUM and Y admits LOW/HIGH. The same LockSet and stable ReadyQueue used by the benchmark decide ownership, handoff and effective priority. An interrupted or returning owner goes to the front of its priority. A yielding worker goes to the back. After each grant SVC returns zero in x0, including the saved acquire frame on direct handoff. The one allowed yield is at 128 completed chunks in the ceiling app.

The timer start sample is taken at the tail of the successful LOW final-lock SVC, before the assembly restore. The programmed compare is start plus CNTFRQ/10000. The next restore timestamp measures the intervening overhead. Eligibility requires an actual IRQ in the linked protected computation while LOW holds the resource. Inspect saved PC against the exact ELF, and reject an IRQ in handler/return overhead or the midpoint yield even if the progress flag was set. Counter samples include their own instructions and observer overhead. There is no retry or timer rearm. For each configuration preserve the first plain and external-trace launch, ELF, argv, clock, raw UART, process status, termination and exception log if present. A host timeout is a retained incomplete observation.

## MUTEX RAW 1

All numeric fields are unsigned hexadecimal words. NA is all ones. Output begins at terminal stop with MUTEX RAW 1 and ends with MUTEX END. The final exit can record IDLE as selected while the guest prints and parks, so that transition has no subsequent restore. A partial dump, missing header/footer, unexplained count, missing restore or overflow cannot qualify.

| Row | Fields in wire order |
| --- | --- |
| H | ceiling flag, terminal reason, number of E rows, number of R rows, completed chunks |
| T | F, arm start, requested CVAL, CVAL readback, arm CTL, raw IAR, IRQ CTL, IRQ CVAL |
| S | task ID, base priority, inclusive stack bottom, exclusive stack top, frame size |
| E | vector entry counter, decision-record counter, op, resource/arg, caller, successor, outgoing frame/PC/SPSR/x19, selected frame/PC/x19, runnable mask including selected, X owner, Y owner, X wait mask, Y wait mask, effective priorities in ID-order bytes with LOW in the least-significant byte, operation result, completed chunks, in-work flag, exited mask, outgoing TPIDR_EL1, selected TPIDR_EL1, selected saved x0 |
| R | assembly restore frame, ELR and counter sampled before final GPR restoration and ERET |
| C | completed sample count and cumulative sum at each completed chunk in order |
| B, HIGH | protected version/count/sum and the snapshot actually read by HIGH |
| A | LOW audit version and completed chunks under X (ceiling only) |
| M | audit version/chunks read by MEDIUM and its independent sum |
| K | LOW after final acquire, LOW after commit, LOW after audit, HIGH before acquire, HIGH after actual successful acquire return, HIGH after read, MEDIUM after audit read, MEDIUM after sum, LOW handler after lock transfer to HIGH |

E operations 1/2/3/4/5 denote acquire, release, yield, exit and IRQ. A blocked acquire has result NA until the release handler transfers ownership, patches its saved x0 and dispatches. Compare R to the actual external exception return where available. Stack bounds apply to the entire saved frame. IRQ entry to HIGH's attempt, HIGH's block to K[8], K[8] to K[4], and LOW's actual acquire/commit/unlock are distinct intervals. Pre-SVC K[3], block entry and block decision bracket different instructions. Unreached K samples are zero. Replay priorities and queue membership, all 256 cumulative products, actual data version/reads, medium sum, exits and resumes independently. The IRQ PC and interrupted register must match the first subsequent LOW resume. A terminal reason of zero records guest completion and defers the independent assessment.

Terminal reasons 1–4 indicate runtime initialization, invalid enqueue, empty selection and frame identity/range respectively. 5 means journal overflow, 6/7 invalid acquire/release, 8 unexpected yield, 9 invalid exit, 10 invalid SVC, 11 failed worker return, 12 returning from exit, 13 timer arm, 14 IRQ identity/readback, 15 missing release/block/handoff, 16 release outside work, 17 unexpected vector/restore overflow, 18 incomplete result or restoration. The main decision ledger holds 32 entries and the assembly return ledger holds 64. The chunk ledger holds 256 completed records. EVD-018 describes the earlier fixture. Product lock sets, physical maximum blocking, WCET and board interference require separate evidence.

[한국어](ordinary-mutex-runtime.md)

## Traceability and pending first verification

CHG-067 affects REQ-002, REQ-005 and REQ-023. DES-003 and DES-012 cover scheduling, frame return and priority changes under contention. The implementation binds these designs to two manifests, the shared lock policy and the ordinary Rust dispatcher. [CHG-067 design](ordinary-mutex-plan.md) · [Manifests](../kernel/src/ordinary_mutex_manifest.rs) · [Lock policy](../kernel/src/mutex_policy.rs) · [Dispatcher and workers](../kernel/src/ordinary_mutex.rs)

First verification is pending under VER-002, VER-005 and VER-023. The native manifest test exercises handoff and priority restoration. Independent verification must retain the first four captures, compare every transition and chunk with its replay, and record the terminal reason and any missing observation. [Native test source](../tools/scheduling_tests.rs) · [Verification plan](verification-plan.en.md)
