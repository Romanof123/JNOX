# Pi 4 MG2 migration across six first boots

M finished before the migration IRQ in both first boots of every placement regime. **MG2 qualified 0/6 first positions.** Historical MG1 qualified **1/6**, partitioned position 1.

Per-CPU exception logs show M’s terminal SVC on CPU0 and the opening P1 SVC on CPU1. The UART 0x1d branch checks M’s sum of 2147516416. Migration return PC, X21 and all four activation summaries were unobserved. Each first boot ran on QEMU 11.1.3 raspi4b.

| Placement | Positions | CPU0 M SVC ELR | CPU1 P1 SVC ELR |
|---|---|---|---|
| global | 1, 2 | 0x81f9c | 0x82098 |
| partitioned | 1, 2 | 0x81fa0 | 0x8209c |
| clustered | 1, 2 | 0x8204c | 0x82148 |

The synthetic host control passed. All seven mutations were rejected when changing count, digest, index, sum, lease or reserved field, or removing a summary. The archive includes offline replay instructions and original/public hash provenance.

[MG2 contract](../boards/pi4-smp-migration.en.md) · [MG1](pi4-migration-first.en.md) · [JSON](pi4-migration-mg2-first.json) · [ZIP](pi4-migration-mg2-first.zip)

[Integrated image verification / 통합 이미지 검증](pi4-mg2-integrated.en-ko.md)
