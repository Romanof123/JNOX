# OFF-only observer cost · Third cohort

[#144](https://github.com/Romanof123/JNOX/pull/144) · [CHG-069](../permit-observer-cost-plan.md) · [v1](permit-observer-first-partial.en.md) · [v2](permit-observer-v2-preflight/extra-control-result.json) · [JSON](permit-observer-cost-v3.json) · [ZIP](permit-observer-cost-v3.zip)

All 16 first launches completed the recorded OFF-only observation, yielding eight adjacent pairs. Suppressing external PL061 read logging gave mixed changes in first-service, phase and IRQ timing.

F enabled the full PL061 trace. R suppressed per-read events. Times use the 62.5 MHz guest counter. Positive differences mean R took more ticks. IRQ lateness and phase delay refer to the first observed event in each launch.

| Positions in launch order | Service F / R | Service R−F | Phase R−F | IRQ R−F |
|---|---:|---:|---:|---:|
| 01-rm-off-1-F / 02-rm-off-1-R | 19672 / 21342 | +1670 | +4130 | +1873 |
| 03-rm-off-1-R / 04-rm-off-1-F | 20354 / 32313 | +11959 | +18967 | +3578 |
| 05-rm-off-2-R / 06-rm-off-2-F | 24228 / 21740 | -2488 | -788 | -957 |
| 07-rm-off-2-F / 08-rm-off-2-R | 18920 / 22761 | +3841 | +6459 | +1642 |
| 09-dm-off-1-F / 10-dm-off-1-R | 22716 / 26994 | +4278 | +1001 | -2354 |
| 11-dm-off-1-R / 12-dm-off-1-F | 23666 / 19835 | -3831 | -4929 | -384 |
| 13-dm-off-2-R / 14-dm-off-2-F | 20727 / 22095 | +1368 | +1377 | -900 |
| 15-dm-off-2-F / 16-dm-off-2-R | 19833 / 21438 | +1605 | +5173 | +692 |

First service and phase delay each increased in six pairs and decreased in two. IRQ lateness increased in four and decreased in four. All 16 stopped before the first publication. Position 03 recorded first fault 107; the other 15 recorded 103. Each guest skipped seven sender opportunities. The frozen inputs, per-position UART and mechanism traces, first-fault operands, retained-file manifests and the cohort-close receipt are in the archive. No output enable or command delivery was observed. Across the corpus, guest rows 80/81/82 record 33 zero write requests and their zero before/after samples (three in position 03, two in each other launch). These samples do not independently locate an external edge or prove which write caused a zero reading. External PL061 events have no assigned guest tick, and R intentionally omits read records. These data cannot establish a synchronized device latency, a causal cost reduction, physical safety or product approval.

Preflight exercised live guard vectors, termination/retention controls and exact-membership checks at source, stage and final. The earlier v1 launch lost most of the mechanism stream (1/16, zero pairs); v2 stopped at preflight on an accepted unexpected file (zero guests). Both retain their original outcomes.

For privacy the public metadata replaces the native and replica cohort path prefixes, operator home prefix and machine hostname with placeholders. inventory.json identifies each original and public file by size and SHA-256, plus empty directory names and synthetic symlink targets. The public archive represents control symlinks as inventory entries to avoid creating links on extraction. Per-position UART, mechanism, stdout, stderr and the four ELFs retain their exact original bytes. The unrelated full source checkout is available via the pinned commit.

```sh
unzip permit-observer-cost-v3.zip -d v3-record
(cd v3-record && sha256sum -c SHA256SUMS)
```
