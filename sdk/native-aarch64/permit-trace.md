# Native permit qualification journal

SVC 62 takes job, phase, stage and absolute physical-counter target in x0 through x3. Phase 1/2 identifies controller A/B (slot 1), phase 3 sender (slot 0), and phase 4 supervisor (slot 2). Stage 1 sleeps until a future target using the scheduler event's counter sample. An already due target polls immediately. Stages 2 and 3 mark user start and end of application work. Zero means success. The application owns job numbering and target selection.

COUNTS reports counter frequency, attempts, rows retained and rows lost. ROW contains the append index followed by twelve unsigned words. Signed status values use two's complement. Slots absent from scheduler selection are UINT64_MAX.

| Kind | Words after kind, in order |
|---|---|
| 1 | PID, job, phase, stage, target, vector sample, scheduler time, saved user PC, status, 0, 0 |
| 2 | reason, scheduler time, before event, after selection, old runnable mask, new runnable mask, old PID, selected PID, exit flag, pre-user-return counter, return PC |
| 3 | vector sample, register sample time, raw GIC IAR, physical CVAL, physical CTL, interrupted PC, saved PSTATE, idle-frame flag, 0, pre-IRQ-return counter, return PC |
| 4 | PID, SVC, input command word, vector sample, scheduler time, after service, status, 0, 0, 0, 0 |
| 5 | before programming, after readback, enabled requested, requested target or zero, physical CVAL, physical CTL, 0, 0, 0, 0, 0 |

Use mask transitions to bound readiness and selection inside the event bracket. Row 2 includes changes, exits and SVC 62 boundaries. Row 5 records changes to requested compare values. Row 3 includes idle and user IRQs; decode the interrupt ID before attributing any interval to the physical timer.

Vector samples follow exception entry. Return samples precede architectural ERET. A subsequent exception from the selected user context gives an upper bound on resumed execution. A zero return cell has no recorded return. The start/end markers delimit application work. For a whole segment include housekeeping through its next actual block or exit, and examine interrupt and dispatch brackets separately. Missing observations, terminal work after a marker and journal overflow limit conclusions. Finite counter observations supply neither a hardware interrupt assertion timestamp nor a product WCET.

Safety EVENT 12 records every Session::check in qualification and named boundary builds. Its eight data words are the original decision tick, caller, generation, accepted sequence, last issue, prior fault, resulting fault and gate state. Caller is the control command for its entry check, 0x102 for the first commit check and 0x202 for the second. Sequence zero means no accepted permit. Inspect both fault flags, monotonicity and prior issue before classifying an observation as fresh. The separate journal timestamp follows the check. Require zero safety journal losses.
