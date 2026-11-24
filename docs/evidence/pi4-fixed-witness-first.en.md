# Pi 4 fixed witness · First model rejections

[#146](https://github.com/Romanof123/JNOX/pull/146) · [#143](https://github.com/Romanof123/JNOX/issues/143) · [JSON](pi4-fixed-witness-first.json) · [ZIP](pi4-fixed-witness-first.zip) · [CHG-063](../boards/pi4-fixed-runtime.en.md#chg-063)

Both first engineering sessions reached terminal export on the raspi4b model. The shipped decoder rejected both with exit 1 (terminal idle mismatch).

Four cold boots, FIFO ordinary/engineering and RR ordinary/engineering, retained matching workload prefixes by policy. The virtual counter ran at 62.5 MHz on core 0 at EL1. Independent same-boot IRQ/SVC/vector/ERET ledgers matched all 10 FIFO and 7 RR generations. In FIFO, HIGH interrupted A at generation 4 and A resumed at 5, progressing from 15,257,564 to 15,257,565. In RR, HIGH ran at 2 and A resumed at 3, progressing from 18,722,743 to 18,722,744. Peer IRQ selections occurred at 4 and 5 and B recorded further work at 6.

The terminal check compared a frame address with the idle entry address. The selected ELR at index 10 actually matched the entry. A later resume check also used the wrong triple. Selected frame/ELR/SP occupy indices 9–11. Both shipped control scripts stopped before mutations. In a separate exercise all 16 mutations for each policy were rejected, including a valid missing-work variant. The decoder therefore supplied no positive control. All 12 invalid Cargo selectors were rejected.

Engineering sample counter minus the dispatch record counter (sample index 6 minus event index 24) ranged from 2,029 to 23,009 ticks for FIFO and 236 to 20,113 for RR. Sampled stack depths were 144–176 and 160–192 bytes respectively. Ordinary boots lack matching service endpoints. These brackets do not establish instrumentation overhead, maximum stack use or WCET. Engineering raw image size was 20,768 bytes, versus 16,672 ordinary. BSS was 101,560 versus 81,992 bytes. The linked reserved stack section was 16,384 bytes in both.

Against the immediate parent built at the same path, both ordinary raw and ELF hashes changed. Two bytes in each raw image map to fixed_runtime.rs source-line records (195→210 and 181→189). The historical EVD-031 model references have their own hashes. Reinterpretation of selected columns or a future post-return interval requires separate evidence on the original bytes. Physical capture and release acceptance remain open. Restore the originals below; the pinned FIFO command reproduces the recorded rejection.

```sh
unzip pi4-fixed-witness-first.zip -d witness
python3 witness/restore.py
cd witness/restored
python3 tools/pi4_fixed_witness.py fifo-eng/first/uart.bin --profile pi4-fixed-fifo-witness --manifest fifo-eng/first/manifest.json --elf fifo-eng/first/kernel.elf --image fifo-eng/first/boot/kernel8.img
```

The original decoder returned 1 on both archived UARTs. The corrected decoder returned 0 on the same FIFO and RR bytes. The replay launched no guests. [Corrected Pi 4 witness decoder · Host replay](pi4-fixed-witness-replay.en.md)
