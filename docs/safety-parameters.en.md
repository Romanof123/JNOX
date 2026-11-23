# ADR-001 · Loss of input and output inhibition on the virtual bench

[한국어](safety-parameters.md) · English

**Decision.** The Functional Safety Team fixes the following values as design inputs and acceptance criteria for the first bench. The scope is QEMU `virt`, one Cortex-A72 core and a virtual `ENABLE` output connected only to the simulation. Status is **bench criteria fixed · test pending**.

## Bench values and rationale

| Parameter | Design value | Rationale and acceptance |
| --- | --- | --- |
| Input | 1 permit every 10 ms | The synthetic sender supplies the current generation, consecutive sequence number, issue time and permit bit. The bench controls the issue times and generation. |
| Control task | 5 ms period and 5 ms release-to-completion deadline | This gives two nominal checks per input period. The actual interval between completed freshness checks must also be at most 5 ms, including scheduling jitter. |
| Expiry | Age since issue ≥ 20 ms | A permit expires two input periods after the last valid permit was issued. At the exact boundary, evaluate expiry before processing a simultaneous arrival. |
| Detection budget | `t_detect − t_fault ≤ 25 ms` | Loss immediately after the last permit allows 20 ms until expiry and up to 5 ms until the next check. |
| Inhibition after detection | `t_safe − t_detect ≤ 5 ms` | The virtual device must reflect inhibition within one control period after detection. |
| End-to-end response | `t_safe − t_fault ≤ 30 ms` | This synthetic test budget adds 25 ms for detection and 5 ms for the subsequent response. Evaluate both intervals and the total separately. |

At boot, `ENABLE=0`. On permit expiry, the controller latches the fault and holds `ENABLE=0`. At startup or during recovery, the supervisor opens a new generation and receives two consecutive permits from that generation at the nominal interval, both with the permit bit set. The output may be enabled only upon a separate explicit rearm command while both permits are fresh. Prior-generation, future-dated, duplicate, out-of-order and expired messages do not refresh the timer or advance the recovery count.

## Independent verification handoff

1. Establish the initial condition `ENABLE=1` using valid input. Record `t_fault` when the injector atomically closes the input gate. Invalidate queued, undelivered permits at this boundary and block subsequent transmissions. Preserve the issue time of the last permit already delivered.
2. Record `t_detect` when the controller sets the fault latch. Record `t_safe` at the first `ENABLE=1 → 0` transition the independent observer sees at the virtual device. Capture the detection marker and the device transition separately. Timestamp injection, detection and device events in ticks on the same monotonic guest virtual timebase, which continues to advance during service loss. Preserve the clock frequency and resolution, event order and raw ticks.
3. Exercise loss immediately after delivery, a freshness check immediately before expiry, and simultaneous expiry and arrival of a new permit. Check for premature inhibition during healthy transmission, that inhibition persists after a fault, and that omitting each rearm condition in turn keeps the output off. Boundary tests of the verdict logic must pass at the limit and fail one tick beyond it. Exclude runs with missing events or timing uncertainty that prevents classification at the boundary from the passing evidence.

Trace `OBJ-001`, `OBJ-004` → `ASM-001`, `HAZ-001`, `HAZ-003` → `REQ-002`, `REQ-003` → `DES-003`, `DES-006` → `VER-002`, `VER-003`. A fresh verifier issues evidence after testing the frozen candidate. Faults in the timebase itself and physical actuator response require separate hardware or HIL verification. [System requirements](requirements.en.md) · [Verification plan](verification-plan.en.md)

### Independent runs for boundary conditions

Reset the starting state for each case. The examples below issue and deliver a permit on the same guest tick. Measure the nominal spacing of rearm permits by their issue times.

| Case | Stimulus and expected result |
| --- | --- |
| Worst check phase | With the output enabled, deliver the last permit at 0 ms and close the gate immediately afterward. Complete a check one tick before 20 ms and the next check 5 ms later. Record the actual completion ticks. Confirm event order `t_fault ≤ t_detect ≤ t_safe` before checking the time limits. Use recorded event order for events on the same tick. |
| Expiry concurrent with arrival | In a separate run, keep the delivery gate open. Deliver a valid permit at 0 ms and skip issuance at 10 ms. Deliver the next valid permit in sequence on the same tick as the check at 20 ms. Expiry takes precedence, the fault latches and the output must turn off within 5 ms. |
| Rearm | With output inhibited, issue consecutive permits in the new generation at 0 ms and 10 ms. A rearm command at 15 ms must enable the output. In a separate run, a first command at 20 ms must leave the output off because the first permit has expired. A command before the second permit is discarded. A new command is required to rearm. |

## Establishing product values

For a given operating situation, the product FTTI runs from fault occurrence to the earliest possible hazardous event if safety mechanisms do not intervene. The Functional Safety Team derives this interval and the safe state from the item definition, operating scenarios and HARA, and a substantiated physical response model or measurements of the actual system. With the Systems Team, it allocates worst-case detection, communication, scheduling and actuator times and their uncertainties, and checks that an explicit margin remains within the shortest applicable FTTI. Review the board safety manual, actual I/O ownership, board or HIL evidence and assumptions of use. The Functional Safety Team submits its rationale and recommendation. Following independent review, the designated item integration authority records the product scope, version and approval decision. Product FTTI, ASIL and physical output inhibition time remain open. [Safety and cybersecurity concept](safety-security-concept.en.md) · [ISO 26262 vocabulary](https://www.iso.org/standard/68383.html) · [ISO 26262 concept phase](https://www.iso.org/standard/68385.html)
