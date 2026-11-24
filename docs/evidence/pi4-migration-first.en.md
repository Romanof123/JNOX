# First Pi 4 worker migration captures

[한국어](pi4-migration-first.md) · [Acceptance contract](../boards/pi4-smp-migration.en.md) · [Capture archive](pi4-migration-first.zip)

One of six first raspi4b model launches met the interrupted-loop replay criteria. Each regime requires two qualifying first launches. Global and clustered scored 0/2 and partitioned scored 1/2.

At source `ba212f67cd75564925a76be0dec8b85aa044fc38`, all three isolated Rust 1.95.0 release builds exited 101 at smp_policy.rs:84 (E0599, missing AffinityDomain::allows). No guest launched. At `6bb8ab9e451f314304b63820b769d4efe90d9183`, all three builds succeeded. A fresh four-vCPU raspi4b process ran each position, two CPUs online, multi-threaded TCG, 30 s ceiling and 1 s terminal quiet. The table gives the half-open arithmetic loop range from each linked ELF and the actual CPU0 exception ELR. The archive contains the complete per-thread exception and return history, linked instructions and linker maps.

| Regime / position | Linked arithmetic loop | CPU0 ELR and instruction | Result |
|---|---|---|---|
| Global 1 | 0x81e10–0x81e34 | No migration IRQ recorded | M finished first, UART failure 0x1d |
| Global 2 | 0x81e10–0x81e34 | 0x81e80, ldp x2,x3,[sp] | CPU1 resumed M after loop return |
| Partitioned 1 | 0x81e14–0x81e38 | 0x81e14, ldr x8,[x21] | CPU0 resumed M inside loop |
| Partitioned 2 | 0x81e14–0x81e38 | 0x81e98, ldr x8,[sp] | Outside loop, MG END |
| Clustered 1 | 0x81ec0–0x81ee4 | 0x81f44, ldr x8,[sp] | Outside loop, MG END |
| Clustered 2 | 0x81ec0–0x81ee4 | 0x81f44, ldr x8,[sp] | Outside loop, MG END |

**Global 2.** CPU0 took IRQ at EL1 with SPSR 0x60000345 and ELR 0x81e80. The task frame at 593392 retained SP 594224. CPU1 took its idle IRQ at 0x80914 then returned to 0x81e80. Separate Linux TID logs establish both CPUs independently. This is an observed cross-CPU continuation outside the qualifying inner loop. At index 51328 the checkpoint bracket crossed leases 1 and 4. At 51344 both ends identified lease 4 and the partial sum was 1318128840. M committed 2147516416 once on CPU1. P0 committed 528, P1 committed 136 and 392. The UART reported local M/P0/P1 totals of 65100/2039/23571 ticks.

**Partitioned 1.** CPU0 resumed at 0x81e14. Lease 1 and lease 5 each contributed local CPU0 service for M, totaling 59390 ticks. P0 and P1 consumed 3976 and 22222 ticks at 62.5 MHz. Index 48960 crossed lease generations. Index 48976 yielded 1199348776 within lease 5. M committed 2147516416. The journal retains sparse checkpoints. Cross-core elapsed latency and physical instructions were unmeasured.

**Offline replay.** Extract the archive and use the measured source's tools with Python and pyelftools 0.32. For each regime and ordinal run `python3 tools/pi4_migration_replay.py <archive>/<regime> <ordinal> <digest>` with the published digest from that regime's session-1.digest or session-2.digest. An unobserved position returns status 1. The first helper invocation lacked pyelftools for the five MG END captures. The dependency-enabled replay used the same six sessions. Account/workspace paths in text and dependent hash references are normalized. provenance.json records original and published SHA-256 values. ELF and raw images match the original freeze, and UART, TID maps and exception logs retain their original bytes. Archive SHA-256 is `75e2245186f24ae7f25e83729ad9358002fdace1aeddc2a7a811ff0c565fac08`.
