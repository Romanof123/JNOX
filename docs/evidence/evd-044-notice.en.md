# EVD-044 · Temporal partition recovery after delayed virtual timer notice

[CHG-046 / IMP-044](../ordinary-partition-capture.en.md) · [#127](https://github.com/Romanof123/JNOX/pull/127) · [9ffd653](https://github.com/Romanof123/JNOX/tree/9ffd653d906b26fcb30e272295c0b43b9acf4839) · [JSON](evd-044-notice.json) · [ZIP](evd-044-notice.zip)

Producer 9ffd653d906b26fcb30e272295c0b43b9acf4839. Four first boots in P-short, P-long, Q-short, Q-long order. Native Linux temporary ELFs, UART and IRQ traces, one Cortex-A72 QEMU at a time, default virtual clock. No recapture. All times below are counter ticks at 62,500,000 Hz (1 ms = 62,500 ticks).

| Case | compare − due | entry − compare | observation − entry | original delay | confirmed A spill per window | recovery | next two | unrelated late edges |
|---|---:|---:|---:|---:|---|---|---|---|
| P-short | 62501 | 29604 | 11720 | 103825 | B → idle 75189 | idle | idle, A | none |
| P-long | 3906250 | 36231 | 13683 | 3956164 | B → idle 3125000, idle → idle 798457 | idle | A, B | none |
| Q-short | 62501 | 15046 | 4929 | 82476 | B → B 74986 | B | B, idle | none |
| Q-long | 3906250 | 17552 | 6383 | 3930185 | B → B 3125000, B → B 796369 | B | idle, A | none |

The loss column gives nominal owner → displaced eligible recipient. The interval between A’s last sample and IRQ entry remains uncertain. All four intentional original-envelope failures recovered. Each capture reached the original frame-four dispatch with no later unrelated timing failure. The independent replay reconstructed direct return PCs, all IRQ ELRs including STOP, the vector, private stacks, worker recurrence, two distinct A samples and release brackets from raw inputs. ELF disassembly shows the previous counter sample copied before the next read and separate current/previous stores. IRQ rows identify PPI 30 and active CNTP_CVAL_EL0, with timer control value 5 and unmasked interrupted PSTATE. This is emulated timer programming. Live worker masking and physical latency remain unmeasured.

Passive #127 P: 10/10 within 1 ms, maximum 42878 ticks.
Passive #127 Q: 13/13 within 1 ms, maximum 26003 ticks.
Legacy #112 partition-bench: PART PASS, IRQs 6, returns 6.
Legacy #112 partition-legacy-diag: PART PASS, IRQs 6, returns 6.
Legacy #112 partition-legacy-diag-negative: PART FAIL, IRQs 0, returns 1. Expected guard 99.
