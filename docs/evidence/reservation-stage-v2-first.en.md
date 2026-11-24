# CHG-071 · Second reservation stage preflight

[CHG-071](../reservation-stage-attribution-plan.md) · [#153](https://github.com/Romanof123/JNOX/pull/153) · [JSON](reservation-stage-v2-first.json) · [ZIP](reservation-stage-v2-first.zip) · [Reference ZIP](evd-053-compiler.zip) · [v1](reservation-stage-first.en.md)

Both baseline load images matched. Both E images failed the required linked instruction order. Zero of twelve planned guests launched.

The aligned-wake (W) and CBS-saturated (S) baselines matched both fresh parent and historical references in all PT_LOAD memory extents and loaded bytes. Their first segments contained 24,858 and 24,866 bytes respectively. Both writable segments contained 1,800 initialized bytes over a 134,704-byte memory extent. The first preflight had stopped earlier on W baseline drift.

In each E image, the final EOI DSB at 0x402013e4 was followed by CBNZ at 0x402013e8. Its taken target was 0x40201424. The fall-through ISB at 0x402013ec and counter read at 0x402013f0 therefore failed the immediate-sample criterion. In the F images the linked listing places ISB/MRS at 0x40201968/0x4020196c. This is static placement only. No run established that either sample executed. The host replay was not performed. No UART, worker outcome or dynamic interval exists for any ordinal.

Restore all 51 manifest entries using the two ZIPs from this directory. The compiler reference ZIP is identical to the published EVD-053 compiler archive. All other entries, including both source tarballs, ELFs, build commands/logs, symbol maps and full disassemblies, are stored losslessly in this supplement.

```sh
unzip reservation-stage-v2-first.zip -d v2
python3 v2/restore.py evd-053-compiler.zip
(cd v2/restored && sha256sum -c SHA256SUMS)
```
