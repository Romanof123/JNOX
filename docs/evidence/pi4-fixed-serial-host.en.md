# Pi 4 serial capture tool · Host qualification

[JSON](pi4-fixed-serial-host.json) · [ZIP](pi4-physical-fixed-host-qualification.zip) · [#142](https://github.com/Romanof123/JNOX/pull/142) · [Source](https://github.com/Romanof123/JNOX/tree/ed611730a657b9cc7dda300626bcc8473faa52c9)

The integrated host collector passed 566 timed checks, eight observer API checks, four simulated-port cases and seven upstream capture tests. [EVD-014](evd-014.en.md) · [EVD-031](evd-031.en.md)

The FIFO and RR replays preserve both archived model UART streams and separate run identities. Both original RR streams were rr-a-first. A constructed pair also exercised rr-high-first. The explicit and default scheduler replays each use the same historical stream twice. EVD-031’s complete archive and members, nine raw EVD-014 records, and all three original distribution ZIPs and their members were authenticated by SHA-256. The CLI checked the original ELF and kernel8.img before replay. A changed ELF or package produced an association discrepancy while preserving the UART match. The archive supplies raw UART and package fingerprints. Full firmware distributions and ELFs are external replay prerequisites.

Bytewise delivery, every single split, malformed frames, both accepted RR orders, an illegal third order, inclusive deadlines, quiet periods, ledger offsets and read failures were exercised. The API checks called the QEMU observer on the host. Injected ports exercised two armings and interrupted reads/input. Three aborted scenarios retained partial identities without final results. The CLI’s physical-mode label in these fixtures describes the selected mode.

The first harness stopped on localized child stderr. A second harness path stopped on an unsupported Python zip keyword. Both original interruptions and their successful retries are archived. Board, EEPROM, card, adapter and witness identity were absent. The study supplies no on-board power cycles, USB observations, executing-PC evidence or WCET measurement.

```sh
sha256sum pi4-physical-fixed-host-qualification.zip
unzip pi4-physical-fixed-host-qualification.zip -d host-records
```
