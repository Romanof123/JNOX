# Corrected Pi 4 witness decoder · Host replay

[JSON](pi4-fixed-witness-replay.json) · [ZIP](pi4-fixed-witness-replay.zip) · [#146](https://github.com/Romanof123/JNOX/pull/146) · [CHG-068](https://github.com/Romanof123/JNOX/commit/90980e3693a09f4be1efdd8440f1620264e1ee49) · [Decoder](https://github.com/Romanof123/JNOX/blob/60ed53d5c14cc33899f57b9285e585edc00daa72/tools/pi4_fixed_witness.py) · [CHG-063](pi4-fixed-witness-first.en.md) · [Inputs](pi4-fixed-witness-first.zip)

The original decoder returned 1 on both archived UARTs. The corrected decoder returned 0 on the same FIFO and RR bytes. The replay launched no guests.

FIFO had 10 selected EL1 returns, 2 IRQs and 7 SVCs. RR had 7 selected EL1 returns, 3 IRQs and 3 SVCs. Each recording also contained one EL2 boot return. Independent replay matched every selected PC and incoming ELR to its external exception ledger. FIFO restored A at generation 5 after HIGH at 4. RR restored A at 3 after HIGH at 2. Terminal idle ELRs were 0x82d70 and 0x82d2c respectively. Each matched the address in its terminal record. The corrected interpretation uses the selected frame, ELR and SP columns 9–11.

Each policy had six independently constructed controls. Five malformed cases (frame used as target, other code target, changed continuation, truncation and unknown tag) returned 1. Removing the resumed progress witness returned 2 for valid but unobserved progress. An additional round-trip positive returned 0 per policy: fourteen controls total. The archive preserves initial host setup errors, the malformed-profile invocation, actual argv, statuses, outputs, mutated bytes and independent report. The first native attempts lacked the observer dependency and then a supported nm. Formal replay used Linux in WSL with GNU AArch64 nm.

The model trace supports the observed returns. The task probe brackets positive progress. The replay supplies no physical-board observation, instruction-retirement measurement or bound on timing. To repeat in a new directory, place both linked ZIPs there and use Python 3 and GNU aarch64-linux-gnu-nm. The commands keep the original replay outputs intact.

```sh
unzip pi4-fixed-witness-first.zip -d inputs
python3 inputs/restore.py
mv inputs/restored pi4-witness-independent
unzip pi4-fixed-witness-replay.zip -d archived-replay
(cd archived-replay && sha256sum -c SHA256SUMS)
cp -R archived-replay repeat
(cd repeat && python3 verify.py)
```
