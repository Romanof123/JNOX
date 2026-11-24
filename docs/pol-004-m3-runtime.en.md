# IMP-042 virtual periodic control candidate

[한국어](pol-004-m3-runtime.md)

Base 33bb739dfc34c3afcf29799ba72d84bf52bcdab6. Admission is refused by the [fixture manifest](pol-004-m3-runtime.json) pending substantiated bounds. This candidate supplies source for independent replay.

## Structure

The existing periodic fixture uses assembly task markers, logical epochs and a timer quantum of CNTFRQ/20. The ordinary profile selects a separate runtime through kernel/build.rs. It shares context.S, Frame, timer PPI/GIC setup, private Stack, ReadyQueue and DispatchPolicy. Control and sender enter Rust on separate stacks. SVC 1 suspends the current control activation and preserves the full frame. The next absolute half-period timer releases that continuation. SVC 2 finishes a job. IRQ handling releases and dispatches. The application owns permit issuance, sampled issuance times, gate state, expiry, rearm and its output writes. Idle independently reads back PL061 GPIODATA bit 0. This is QEMU peripheral-register observation only.

## Replay

Use the repository-pinned Rust 1.95.0, target aarch64-unknown-none. Select one policy and case per clean boot. Archive the exact candidate diff/tree, runtime manifest, executable ELF SHA-256, compiler information, QEMU executable hash/version, argv, timer mode, stdout and stderr for each run. From the repository root on Linux:

```sh
cd kernel
export JNOX_BOARD=virt JNOX_PROFILE=periodic JNOX_APP=control-rm JNOX_CONTROL_CASE=loss
unset JNOX_FIXED_TRACE
cargo build --locked --release
elf=target/aarch64-unknown-none/release/jnox-kernel
sha256sum "$elf" "$(command -v qemu-system-aarch64)"
qemu-system-aarch64 --version
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio -kernel "$elf" > rm-loss.uart 2> rm-loss.stderr
```

The guest parks after CONTROL END. Stop that QEMU process after collecting the terminal marker, or retain a timed-out incomplete stream as a failed capture. Run `python3 ../tools/control_observations.py loss rm-loss.uart` in kernel. Repeat independently with control-dm and for healthy, late and demand. For separately declared deterministic fixtures add `-icount shift=0,align=off,sleep=off` before `-kernel`. Record both clock modes explicitly. The arithmetic of one run cannot transfer across modes.

Ordinary runs measure the sender's issue tick on its own stack after dispatch. It targets 0.5 ms into its job. A missed target is retained. Boot inhibited, two qualifying issues, one supervisor command, healthy monitoring, closure just after the fourth delivery, expiry, and the first device readback of the falling transition run in the loss case. The 15 ms command is relative to the first actual issue. A positive case requires actual issue spacing of exactly 10 ms. A nonqualifying ordinary pair is a nonpass. Late holds the IRQ mask through 0.625 ms past target 12. Demand consumes at least 0.3125 ms each activation from phase 12. Both controls require their own cause and independent readback of inhibition.

## Trace

Every row is event, counter, argument A, argument B. File order is the tie identifier. Row 1 records the nominal epoch, CNTFRQ and final phase index. Row 25 identifies the separately scripted clock fixture. No UART operation occurs during scoring.

|Event|Counter and arguments|
|---|---|
|2 / 3|Program completion, phase and absolute compare / IRQ vector-entry sample, requested and actual phase|
|4 / 7|Worker ready, id and nominal release / continuation ready, id and target|
|5 / 6|Dispatch, id and next timer index / exception entry, id and elapsed charged service ticks|
|8 / 9|Activation end, id and cumulative service / job finish, id and nominal release|
|10 / 21|Phase completion upper sample, index and target / decision completed, index and freshness input sample|
|11 / 12|Actual issue, sequence and generation / delivery completion, sequence and actual issue tick|
|13 / 14|Latch set upper sample, cause and last issue / independent device sample, prior and current register bit|
|15 / 16|Timing failure, cause and next phase / gate closure, last delivered sequence and issue tick|
|17 / 18 / 19|Command issue, generation tag and reserved / horizon device bit and scheduler fault flag / boot device bit and reserved|
|20 / 23 / 24|Successful rearm, first and second issue ticks / gated sender attempt, phase and reserved / rejected input, sequence and reserved|
|32 / 33 / 34 / 35 / 36|Embedded policy and case hash / longest worker or idle masked body bracket and duration / IRQ callback interval and selected worker / SVC callback interval and selected worker / receiver sequence and issue after silence or invalid input|
|26 / 27 / 28 / 29 / 30 / 31|One-slot saturation / readback bracket / latch bracket / gate bracket / delivery bracket / recovery generation and eligibility|

Bracket rows use their lower tick as the counter field and upper tick as argument A. For row 26 the arguments are rejected and retained sequence. Causes are 0 permit expiry, 1 late or missed timer, 2 unavailable continuation, 3 service budget, 4 overlap, 5 horizon overrun, 6 deadline, 7 decision envelope and 8 inter-check spacing. IRQ/SVC timestamps are sampled immediately after entering the vector and saving the two scratch registers. The control envelope uses a conservative completion sample. The cost of code before the first entry sample remains measurement uncertainty. A device sample bounds when the observer saw the edge, independently of the write instruction. Lost/invalid traces emit CONTROL INVALID, or lack the terminal marker.

## Scope of accounting and outstanding cases

The service charge starts at the selected task's dispatch tail and ends at the next IRQ or SVC entry sample. It includes restore and task execution plus the vector sampling preamble, excludes suspended time and callback body, and accumulates interrupted slices. SVC callback service, IRQ callback and dispatch, mask regions and device observation must remain separate in analytical accounting. The 250/250/750 µs thresholds monitor recurring service. Assigned thresholds and recorded maxima are not proven WCET. Timer programming, task admission and uncertainty still need independently justified bounds. Longest calibrated blocking, external IRQ bursts, combined maximum interference, post-handoff board operation and transport VER-019 remain open.

The separately selected scripted cases are worst, tie, early-command, expired-command, missing-permit, no-command, recovery, old-generation, invalid, silence and saturation. These run on the same guest counter and device, outside the ordinary worker profile. Classify the actual measured ticks. A sample/check that misses a prescribed tick, or a nonzero bracket spanning the required tie, leaves that boundary inconclusive. Worst and tie currently *attempt* target timestamps. Their computation after reaching a target can miss the required completion/arrival tick. Script saturation and semantic rejects only exercise bounded stimulus state. Independent replay and exact-boundary success are pending.


The scorer accepts each scripted case name and its raw UART path. Run `python3 tools/control_observations.py verdict-boundaries` from the root for the deliberately synthetic equality and one-tick-over verdict arithmetic. In a guest capture, the scorer checks the actual rearm issue and command ticks, consecutive boundary decisions, independent readback, unchanged receiver state and terminal observation. An inconclusive exact-tick attempt returns failure. The ordinary scorer also reports actual service per 10 ms window. Host code and replay commands are candidate inputs for independent QA.

The IRQ/SVC interval ends just before recording that interval and transferring to a task. Its small logging tail and restore preamble need bounded attribution before analytical admission. The longest masked-body row measures wall time inside the masked worker/idle closure, excluding mask entry/exit instructions. These observations are lower bounds on those overheads. Missing a complete phase aborts the trace after one current evaluation on the control stack and an off-state readback; a partial horizon cannot pass.

## Injector-to-sender handoff

Select JNOX_CONTROL_CASE=handoff-healthy or handoff-loss under either policy. Compile, capture and score the full case name with its own ELF hash and clock mode. Keep the ordinary-unassisted healthy/loss and scripted-only boundary runs separate.

The idle supervisor initially owns generation 1. With IRQs masked it prepares a bounded receiver slot, samples the actual issue tick, publishes the timestamp into that slot, and samples the post-store tick after a barrier. Both must equal the programmed instant at epoch +0.5 ms and again 10 ms later. At +15 ms relative to the first permit it issues the explicit command. The ordinary control worker evaluates it. Idle independently reads the PL061 rising transition before transferring sole ownership. The first two ordinary sender activations abstain. At its next release the sender takes generation 1, sequence 3. In the loss case it actually delivers sequences 3 and 4 before closing the gate.

Rows 37 through 42 distinguish pre-owner sender abstention, disqualified target, successful transfer (generation and next sequence), injector sample (sequence or command index and programmed target), injector issue (sequence and generation), and injector delivery (sequence and issue tick). A missed issue, post-store receipt or command tick disqualifies setup for this cold boot. Preserve all raw samples. In the handoff profile, rows 11/12 belong only to the ordinary sender. A transfer additionally requires actual independent device readback after the control worker accepts the command.

First independent runtime verdict, PARTIAL. [EVD-042 v1](evidence/evd-042-v1.en.md) · [한국어](evidence/evd-042-v1.md)

## Cold-start observation revision

The first default-counter captures for d17ce2d recorded RM loss programming phase zero 2.382 ms before the epoch and entering the first timer vector 2.547 ms after it (requested index 0, observed index 1). RM healthy armed 2.337 ms early, reached the first vector 1.972 ms late and subsequently recorded further late entries. Those captures remain failed evidence. They establish timely initial compare programming. They cannot attribute the delivery delay to host scheduling, translation, CPU masking or a specific QEMU path.

This revision initializes runtime state, constructs the private idle frame and performs the first exception return before choosing its one immutable epoch. The lead remains one 2.5 ms phase. Ordinary idle spends the interval between pending jobs in an IRQ-enabled WFE; a timer IRQ sends SEV so a wake arriving between the idle decision and WFE is retained on the single PE. Idle polls only when an explicit supervisor subphase must occur before the next timer. The late-IRQ negative still deliberately masks across its programmed deadline. The new event instructions and trace stores are part of this distinct executable and must be charged in its own capture.

Raw row 43 samples first private idle-stack entry, with frequency and phase length. Row 44 samples idle preparing for an unmasked wait, with the next requested index and sampled DAIF. Row 45 records the interrupted worker and saved return PC at the vector sample; row 46 records saved DAIF mask bits and the requested phase at that same sample. Correlate the PC with the exact ELF and compare rows 1/2/3/43/44 for setup, programmed lead and vector lateness. No warmup release or discarded first job exists. Passing still requires the original envelopes and actual events on every scored job. This revision is a source candidate awaiting separate first-cold verification.

## Exact scripted boundary observations

Only the separately selected worst and tie cases use the compact AArch64 primitive. It reads the live physical counter, compares the previous accepted issue with expiry, conditionally stores the real fault latch and reads the counter after completion. The tie then separately samples issue, publishes the prepared fourth permit and samples the completed ingress store. Equal counter samples on both sides of each operation and source instruction order establish a tie at counter resolution. Tracing and the GPIO write/readback follow these operations. Event 47 carries the ingress interval and sequence. The normal receiver consumes and rejects that permit after the latch. Event 12 represents accepted state and ends at sequence 3. The old scorer simultaneously required expiry-first inhibition and a fourth accepted permit, although this latch disables rearm and makes normal delivery reject it.

Use the frozen deterministic clock. A timestamp outside its required tick fails even when the lateness is one tick. Preserve first captures including failures. Independent verification must establish both boundaries and the device response from the raw samples. These scripted paths supply no ordinary worker scheduling evidence.

For independent replay use the Replay build command above with JNOX_CONTROL_CASE=worst or JNOX_CONTROL_CASE=tie, separately for control-rm and control-dm. Add the documented -icount option to each QEMU invocation and retain unique UART/stderr files. From kernel, score each capture with `python3 ../tools/control_observations.py worst rm-worst.uart` or `python3 ../tools/control_observations.py tie rm-tie.uart`, substituting its actual filename.

In the worst-phase script the last permit is accepted while the queue is empty. The same instruction block closes the gate before tracing. Rows 12 and 16 must identify sequence 3. Both ends of the row 29 closure bracket must equal the anchor tick. A later closure fails even if the response intervals alone fit. Row 17 records a supervisor command arrival and row 20 records successful rearm.
