# EVD-055 · Integrated ordinary proportional gate

Three first default-clock QEMU boots completed on the integrated source. The first EVD-055 cohort is counted separately.

[JSON](evd-055-integration.json) · [ZIP](evd-055-integration.zip) · [#136](https://github.com/Romanof123/JNOX/pull/136) · [Source](https://github.com/Romanof123/JNOX/tree/589e927bdc2e21ed68de1315d030d6231d18bacb)

| Case | A/B/C grants | A/B/C whole service ticks | IRQ/SVC/ERET |
|---|---|---|---|
| lottery1 | 10/8/14 | 10/8/14 | 33/0/33 |
| lottery2 | 5/11/16 | 5/11/16 | 33/0/33 |
| stride | 6/6/4 | 6/12/8 | 11/6/17 |

At 62.5 MHz, lottery and stride ticks span 625,000 and 2,500,000 counter counts respectively. The scorer reproduced the seeded draws and linked exception order and return PCs, including idle, against each guest’s ELF. Stride’s first eight grants yielded 4/8/0 ticks, then 2/4/8 after C joined at the incumbent floor. Integer service, sampled progress and frame fingerprints come from the guest. The external trace establishes exception and return addresses.

All five whole ELF hashes changed from the references. Every PT_LOAD and .text comparison matched for the three ordinary images and the RM marker and baseline images. The two RM images were build comparisons only. Five invalid selectors (control scenario, board, app, profile and mixed benchmark) were rejected in both Cargo and standalone selector execution. All 130 native scheduling tests passed. The first RNG witness harness omitted feedback_policy and failed to compile. The corrected witness rejected zero, accepted e220a8397b1dcdaf on the second word and mapped ticket 2 to B. Both sources and compiler logs are archived.

The gate found zero evidence-audit findings, checked 34 original archive hashes and replayed all three previous captures. The lottery-miss and stride-delay diagnostic arms remain unexecuted. The four-IRQ incident under #79 and physical/product timing remain open. To recover the captured bytes and replay the three integrated runs with Python 3:

```sh
unzip evd-055-integration.zip -d integrated
cd integrated
sha256sum -c FILES.sha256
for k in lottery1 lottery2 stride; do
  python3 scorer/score.py score "$k" "guests/$k/uart" --elf "guests/$k/kernel.elf" --exceptions "guests/$k/exceptions" > "$k-replay.json"
done
```
