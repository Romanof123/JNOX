# Response-time analysis for RM and DM

[한국어](fp-response.md) · English

CHG-010 allocates IMP-008 to POL-004, REQ-002 and REQ-023 under DES-003 and DES-012. VER-002 and VER-023 check the analysis separately from the guest that demonstrates periodic job dispatch. Functional-safety and platform teams supply the bounds for a specified board, clock, build and workload. The tool runs on the development host. [Live RM/DM contract](m1-periodic.en.md) · [Scheduling release cases](scheduling-policies.en.md)

Run `python tools/fp_response.py docs/examples/rm-dm-workload.json` from the repository root. The example fixes the execution costs and deadlines of two tasks and selects DM. Changing only `policy` to `rm` makes the shorter-period task run first. [Example input](examples/rm-dm-workload.json) · [Python tool](../tools/fp_response.py)

## Inputs and assumptions

- Use one initially idle, fully preemptive CPU and independent sporadic tasks. All times are integer ticks on one time scale. Each task has a unique ID, a positive minimum interarrival time `period`, relative `deadline` at most its period, and positive worst-case execution cost `wcet`.
- Optional `dispatch` is the upper bound on all dispatch and preemption costs attributable to one job. Optional `blocking` bounds its total lower-priority blocking. Optional `jitter` bounds the delay from nominal arrival to readiness and must be less than its deadline. All three default to zero. The caller must cover scheduler and hardware costs for the chosen configuration. Self-suspension and migration require another model.
- Each entry in `irqs` supplies a unique ID, minimum interarrival time, execution cost `wcet` and optional release jitter. Every modeled IRQ can preempt each task. Count IRQ CPU service once across the input budgets.
- RM orders tasks by increasing period and DM by increasing relative deadline. Equal keys in the selected policy require an explicit tie and interference model and are rejected. Duplicate IDs or JSON keys and fractional, negative or Boolean durations are invalid. A zero execution cost, period or deadline is invalid.

## Calculation and interpretation

For task *i*, start with *w = Cᵢ + Bᵢ*, where *Cᵢ* includes the per-job dispatch cost. At each iteration add the interference of all higher-priority tasks and all IRQ sources to that same base. Source *h* contributes *ceil((w + Jₕ) / Tₕ) × Cₕ*. When the window stops growing, *Jᵢ + w* bounds response from nominal release. Equality with the deadline passes. The limit defaults to 1,024 updates per task. [Fixed-priority response-time bounds](https://www.cs.york.ac.uk/rts/publications/Davis2008a.html)

| CLI exit | Result | Interpretation |
| --- | --- | --- |
| 0 | `within_bounds` | All task bounds fit the supplied deadlines under the stated assumptions. |
| 1 | `deadline_exceeded` | An iterate exceeds a deadline. The tool reports that iterate and leaves the bound empty. An actual miss requires observed execution evidence. |
| 2 | `inconclusive` or invalid input | Iteration stopped before convergence, or the input was rejected with a diagnostic. |

For the example, DM yields nominal response bounds of 1 tick for urgent and 3 for short. Under RM the short task has a bound of 2 ticks and the urgent task crosses its deadline of 2 ticks. Independent checks cover combined jitter, blocking, IRQ demand and dispatch cost, iteration exhaustion and synchronous reference schedules. Evaluate the result for the whole workload. [Acceptance tests](../tools/test_fp_response.py)
