# EVD-053 · Compiler profile comparison

[CHG-064](../reservation-compiler-profile-plan.md) · [#145](https://github.com/Romanof123/JNOX/pull/145) · [EVD-053 v3](evd-053-v3.en.md) · [JSON](evd-053-compiler.json) · [ZIP](evd-053-compiler.zip)

Only one of eight size-optimized normal boots completed. None of eight O2 boots completed. Both profiles refused both overloads. Retain the production size setting and 1 ms guard. Six selectors were built at each setting from the same frozen kernel. All six size builds reproduced the historical ELF hashes. The initial build driver stopped before invoking Cargo because the host Python lacked hashlib.file_digest. Both drivers and the original error are archived.


Size passed 1/8 and O2 passed 0/8 normal attempts. Qualification requires 8/8. All four R runs refused 9/8 before first timer programming and dispatch.

| Pair | First | Case | S status | O2 status | S first marker - E | O2 first marker - E | O2-S IRQ to precommit | O2-S source end to precommit | Gap (s) |
|---|---|---|---|---|---:|---:|---:|---:|---:|
| 1 | S | aligned-wake/plain | [2, 5] | [2, 5] | unobserved | unobserved | -3490 | -1047 | 0.869876 |
| 2 | O2 | cbs-wake/trace | [2, 5] | [2, 5] | unobserved | unobserved | 3673 | 4532 | 0.644008 |
| 3 | S | aligned-saturated/trace | [2, 2] | [2, 5] | 67380 | unobserved | 6163 | 6522 | 0.538865 |
| 4 | O2 | cbs-saturated/plain | [2, 5] | [2, 5] | unobserved | unobserved | 12995 | 9597 | 0.571169 |
| 5 | S | aligned-wake/trace | [2, 5] | [2, 5] | unobserved | unobserved | 4910 | 5047 | 0.634177 |
| 6 | O2 | cbs-wake/plain | [2, 5] | [2, 5] | unobserved | unobserved | 4325 | 4837 | 0.553792 |
| 7 | S | aligned-saturated/plain | [2, 5] | [2, 5] | unobserved | unobserved | -2661 | -602 | 0.570353 |
| 8 | O2 | cbs-saturated/trace | [0, 0] | [2, 5] | 58439 | unobserved | 6418 | 5404 | 0.584974 |
| 9 | S | aligned-refused/plain | [1, 2] | [1, 2] | unobserved | unobserved | unobserved | unobserved | 0.552533 |
| 10 | O2 | cbs-refused/plain | [1, 2] | [1, 2] | unobserved | unobserved | unobserved | unobserved | 0.547714 |

Each pair has one observation per setting. Timing covers cold QEMU translation, host scheduling and the guest instrumentation. Trace and plain have separate rows. External exception/return verification is available in trace only. Original cohorts remain separate: compact v3 4/8, its matched v2 5/8, historical v2 1/8 and each historical v1 0/8.

observations.json contains per-grant actual marker, interval, deadline charge, stack/PC, recurrence, cutoff and later service checks. pairs.json retains stage operands in ticks, stages absent from a receipt are null. meta.json gives UTC starts/ends, actual argv, host loads and process status. footprints.json gives per-ELF bytes and reference headroom. Normal images reserve 704 bytes across two banks, witness and receipt, included in linked totals. Physical high-water and physical timing remain unknown.

## Normal opening and later service

All tick offsets below use the guest virtual counter. The service bound for each worker grant includes the sampled pre-ERET to vector interval. Each allocation charge is the sum for its actual owner and deadline.

| Ordinal | Arm and case | Terminal slot/status/reason | Receipt flags | Guard operands | First marker - E | Later A/B grants | Largest worker offset | Largest allocation charge |
|---|---|---|---:|---|---:|---|---:|---:|
| 1 | S/aligned-wake/plain | 0/2/5 | 7 | [3214682, 3151741] | unobserved | 0/0 | unobserved | unobserved |
| 2 | O2/aligned-wake/plain | 0/2/5 | 15 | [3237127, 3173476] | unobserved | 0/0 | unobserved | unobserved |
| 3 | O2/cbs-wake/trace | 0/2/5 | 15 | [3243352, 3179475] | unobserved | 0/0 | unobserved | unobserved |
| 4 | S/cbs-wake/trace | 0/2/5 | 3 | [3246334, 3181873] | unobserved | 0/0 | unobserved | unobserved |
| 5 | S/aligned-saturated/trace | 0/2/2 | 31 | [3248972, 3181592] | 67380 | 0/0 | 67380 | 3008907 |
| 6 | O2/aligned-saturated/trace | 0/2/5 | 15 | [3256082, 3187767] | unobserved | 0/0 | unobserved | unobserved |
| 7 | O2/cbs-saturated/plain | 0/2/5 | 15 | [3246739, 3177703] | unobserved | 0/0 | unobserved | unobserved |
| 8 | S/cbs-saturated/plain | 0/2/5 | 7 | [3252369, 3188032] | unobserved | 0/0 | unobserved | unobserved |
| 9 | S/aligned-wake/trace | 0/2/5 | 15 | [3237590, 3173387] | unobserved | 0/0 | unobserved | unobserved |
| 10 | O2/aligned-wake/trace | 0/2/5 | 7 | [3237635, 3173714] | unobserved | 0/0 | unobserved | unobserved |
| 11 | O2/cbs-wake/plain | 0/2/5 | 15 | [3241448, 3177341] | unobserved | 0/0 | unobserved | unobserved |
| 12 | S/cbs-wake/plain | 0/2/5 | 15 | [3237702, 3174040] | unobserved | 0/0 | unobserved | unobserved |
| 13 | S/aligned-saturated/plain | 0/2/5 | 15 | [3245851, 3177563] | unobserved | 0/0 | unobserved | unobserved |
| 14 | O2/aligned-saturated/plain | 0/2/5 | 15 | [3241383, 3177927] | unobserved | 0/0 | unobserved | unobserved |
| 15 | O2/cbs-saturated/trace | 0/2/5 | 15 | [3242559, 3177419] | unobserved | 0/0 | unobserved | unobserved |
| 16 | S/cbs-saturated/trace | 10/0/0 | 31 | [0, 0] | 58439 | 3/4 | 58439 | 3067677 |

The RAW3 flag mask records IRQ, precommit sample, publication/readback, cutoff readback and selection in successive bits. Status 2/5 stops before selection and compares its sample to the slot opening plus 62,500 ticks. Status 2/2 follows a measured late worker marker. A result of 0/0 reached the complete modeled horizon. For a failing run, all stages after the last receipt remain unobserved.

## Footprint

| Selector | S code+constant | O2 code+constant | S mapped RAM | O2 mapped RAM | O2-S RAM |
|---|---:|---:|---:|---:|---:|
| aligned-refused | 8205 | 9881 | 92721 | 94393 | 1672 |
| aligned-saturated | 24862 | 29490 | 159570 | 164194 | 4624 |
| aligned-wake | 24858 | 29522 | 159562 | 164226 | 4664 |
| cbs-refused | 8205 | 9881 | 92721 | 94393 | 1672 |
| cbs-saturated | 24862 | 29490 | 159570 | 164194 | 4624 |
| cbs-wake | 24862 | 29522 | 159570 | 164226 | 4656 |

All normal images include 65,536 bytes of trace storage, three 16,384-byte task stacks and one 16,384-byte linked boot stack. footprints.json gives exact margins against 256 KiB code+constant and 2 MiB mapped RAM. Those ceilings are reference figures for this instrumented virtual board.

## Interpretation

- irq_to_precommit: 8 pairs with both observations; O2 lower in 2, equal in 0, higher in 6.
- source_end_to_pre: 8 pairs with both observations; O2 lower in 2, equal in 0, higher in 6.
- first_offset: 0 pairs with both observations; O2 lower in 0, equal in 0, higher in 0.

Each ordinal ran once. Process status 124 records the host stopping the halted/idle guest after the fixed budget. Completeness is checked separately by terminal wire framing. Early terminal failures remain in the denominator. The two retained copies and parser outputs refer to the original first captures.

The original source decoder rejected 16/20 captures. The pinned corrected decoder and independent replay agreed on all 20 terminal operands. All five attribution mutations and all five tag-24 role mutations were rejected by the corresponding independent checks. The corrected decoder also rejected each role mutation.

S normal status/reason, stage mask and number of launches: 0/0, mask 31, 1 launch(es), 2/2, mask 31, 1 launch(es), 2/5, mask 3, 1 launch(es), 2/5, mask 7, 2 launch(es), 2/5, mask 15, 3 launch(es).

O2 normal status/reason, stage mask and number of launches: 2/5, mask 7, 1 launch(es), 2/5, mask 15, 7 launch(es).

Ordinal 16 completed 10 slots with 4/4 observed A/B grants. The greatest marker offset was 58439 ticks.

O2 selection gate: rejected; keep the existing default.

## Restore originals

The archive preserves each input under its original relative name via a SHA-256 object index. The ledger covers the independent first cohort.

```sh
unzip evd-053-compiler.zip -d compiler-study
python3 compiler-study/restore.py
(cd compiler-study/restored/reservation-profile-fresh && sha256sum -c SHA256SUMS)
```
