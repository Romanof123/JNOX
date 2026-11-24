# First integrated reservation application boots

2/8 first positions completed their declared horizon. The eight-position gate remains open. All eight used a single AArch64 Cortex-A72 virt/GICv2 CPU, the default 62.5 MHz clock and one unchanged size-optimized ELF per app. The 1 ms limit was 62,500 ticks. All first UART footers were retained. Separate exception histories accompany the four traced runs. Each process remained under the original 15 s recorder limit.

| Position | Application | External trace | Slot / owner | Status / reason | Operands | Mask | Measured grants |
|---:|---|---|---|---|---|---:|---:|
| 1 | aligned-wake | 0 | 34 / 2 | 0 / 0 | 0 / 0 | 31 | 18 |
| 2 | cbs-wake | 1 | 0 / 0 | 2 / 2 | 3226465 / 3160245 | 31 | 1 |
| 3 | aligned-saturated | 1 | 0 / 2 | 2 / 5 | 3253684 / 3184887 | 3 | 0 |
| 4 | cbs-saturated | 0 | 0 / 0 | 2 / 2 | 3217955 / 3153727 | 31 | 1 |
| 5 | aligned-wake | 1 | 1 / 1 | 2 / 2 | 6348592 / 6285864 | 31 | 2 |
| 6 | cbs-wake | 0 | 0 / 2 | 2 / 5 | 3220475 / 3156905 | 15 | 0 |
| 7 | aligned-saturated | 0 | 0 / 2 | 2 / 5 | 3231143 / 3164139 | 3 | 0 |
| 8 | cbs-saturated | 1 | 10 / 2 | 0 / 0 | 0 / 0 | 31 | 8 |

Independent replay checked the compact first decision, per-task frame/PC, actual assembly returns, timer cutoff/readbacks, Rust worker progress recurrence, ownership and accumulated service by allocation deadline. A 2/5 stop records the opening guard before worker selection. A 2/2 stop records an actual worker marker after the opening bound at the stated slot. Normal completion requires the 34-slot wake or 10-slot saturated horizon, work and allocation audit and terminal success. These eight runs keep their own denominator. The separate 24 CHG-081 B/E/F boots and six controls remain in the earlier report.

[JSON](reservation-integrated-first.json) · [ZIP](reservation-integrated-first.zip) · [CHG-081](reservation-chg081-first.en-ko.md)

The integrated ordinary workload, reservation jobs, stage hooks, service and temporal policy source files match reservation parent d6efa4fbf5386d6747c21fc785ecccdf276bf911. Against main 12a98a9178903798b6b90cced17bd785d219eedc, periodic-bench matched as a whole ELF and aperiodic-bench matched its entry and load image. CBS-bench and CBS-wake-bench retained their entry points, but their executable text grew by 8 and 16 bytes; the shared admission routine changed. Their operational behavior remains unverified in this cohort. No legacy guests were launched. [Build identities](reservation-integrated-compat.json)
