# Host replay of the archived aperiodic server captures

[#120](https://github.com/Romanof123/JNOX/pull/120) · [7e0f913](https://github.com/Romanof123/JNOX/tree/7e0f913882342c2d6cba2cbfe38be290f15a2d2f) · [JSON / SHA-256](evd-045-host-replay.json) · [EVD-045 v1](evd-045-v1.en.md) · [EVD-045 v2](evd-045-v2.en.md)

All 44 CLI replays supplied the original UART, same-run exception log, ELF and metadata. The reviewer matched each named image to its build index and environment. There were 21 completed workloads, eight exercised late controls, ten pre-arbitration inhibitions and five invalid incomplete icount captures. Both original default-clock late images stopped before injection.

| Recorded cohort | Completed | Prescribed late | Inhibited | Invalid incomplete |
|---|---:|---:|---:|---:|
| v1 default | 0 | 0 | 10 | 0 |
| v1 diagnostic (icount) | 0 | 2 | 0 | 5 |
| v1 diagnostic-complete (icount) | 5 | 2 | 0 | 0 |
| v2 default | 8 | 2 | 0 | 0 |
| v2 diagnostic (icount) | 8 | 2 | 0 | 0 |

All 26 host controls matched their expectations. Twenty-three rejected corrupt accounting, ownership, service or return data, or incompatible input identity. UART-only and ELF-assisted replays completed within their narrower scopes. A relabeled pre-arbitration capture still reported inhibition because several declared applications share that prefix. External ERET records establish return identity and order. Guest counters establish chronology. This host replay adds no guest boots or physical timing measurements. The difference between the default-clock cohorts remains unexplained.
