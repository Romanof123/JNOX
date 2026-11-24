# Integrated Pi 4 FIFO and RR · Four first model boots

[한국어](pi4-fixed-witness-integration.md) · [Per-image digests and host windows](pi4-fixed-witness-integration.json) · [Complete evidence ZIP](pi4-fixed-witness-integration.zip) · [Tested source](https://github.com/Romanof123/JNOX/tree/aa691462c52bcad40a2b17077bd48e9ddfb9bd09)

One cold raspi4b guest ran per image. All four produced the expected work markers within 30 seconds from host arm and remained quiet for at least one second. Engineering FIFO scored **0 (complete)** and RR scored **2 (valid, peer continuation unobserved)**. The original engineering FIFO and RR UARTs each scored 0 with this same integrated host decoder. These are separate recordings. [First original recordings](pi4-fixed-witness-first.en.md) · [Original replay](pi4-fixed-witness-replay.en.md)

| Image | UART bytes | Final LF / quiet (s) | Core 0 IRQ / SVC / selected EL1 ERET | Later worker evidence |
|---|---:|---|---|---|
| Ordinary FIFO | 175 | 0.666 / 1.011 | 2 / 7 / 10 | After HIGH exit, ERET to 0x8127c, then a producer block at 0x814c8 |
| Ordinary RR | 124 | 0.600 / 1.010 | 3 / 3 / 7 | B entry, then A at 0x813b0 and B at 0x81468 after separate returns |
| Engineering FIFO 63001 | 2152 | 0.616 / 1.010 | 2 / 7 / 10 | Generation 4 selected HIGH. Generation 5 selected A's interrupted frame and recorded advancing work |
| Engineering RR 63002 | 1742 | 0.561 / 1.010 | 3 / 3 / 7 | Generation 3 recorded advancing A after HIGH. Generation 4 recorded B entry and work. Generations 5 (A) and 6 (B) have no new work samples |

Each engineering incoming ELR and selected ELR matched the independent same-guest exception and ERET log, including terminal idle. IRQ IAR 30 comes from the guest. QEMU independently reported CPU zero, IRQ and vector. No other CPU supplied an exception in these logs. The ordinary traces include executed worker translation blocks. The first ordinary RR resume after HIGH fell in the shared work loop outside the worker filter. Later worker blocks identify A and B individually. These observations supply model evidence. Physical firmware execution, IRQ delegation, instruction retirement, latency, WCET and safety approval remain open.

## Ordinary and engineering artifacts

The pinned Windows Rust 1.95.0 compiler built all comparisons at identical source and target paths with a fresh target for each build. The frozen protocol had specified native ext4 builds. Each first guest capture and package originated on persistent user-owned ext4, and each separate durable copy was hash-verified before the following boot. Each image used the pinned firmware/config and a freshly verified package. The model loaded its raw kernel directly.

Compared with ordinary main fb7d942, FIFO and RR both had identical .text, writable PT_LOAD and segment layout. Each 16672-byte raw image differed at two .rodata bytes in the executable PT_LOAD. They encode Rust bounds-check source lines 195→210 and 181→189 in fixed_runtime.rs. Raw offsets are FIFO 0x3800/0x3818 and RR 0x3740/0x3758. Full ELFs also contain different compiler-generated symbol strings. Ordinary images contain no linked witness buffer/exporter symbols. Compared with approved producer c388505 at IDs 63001/63002, all four builds matched .text, raw image and both PT_LOADs; full ELF hashes differed. Engineering raw size was 20768 bytes for both profiles. Each engineering writable segment grew by 19568 bytes in memory. Ordinary UART has no matched service-counter endpoints for an overhead delta.

## Inspect the archive

The ZIP uses SHA-256 deduplication. Extract restore.py from the ZIP and run the command below with a new destination. index.json records the size and digest for every path. UART, ELF, staged firmware and full exception/worker-block streams retain their original bytes. Records containing private host paths have public path tokens and a separate original digest. The pre-build protocol and retained build-path deviation are under provenance/. The first-capture verified.json files refer to original hashes.

```sh
python3 restore.py pi4-fixed-witness-integration.zip restored
```

In restored/integrated/, each arm has a first/ directory containing UART, host read times, launch arguments, exception log, ELF and package. Engineering arms include external-ledger.json, score.txt and mutations/. Ordinary arms include ordinary-ledger.json. main/ and approved/ hold comparison manifests. archive-engineering/, third/ and controls-second/ preserve the archive replays and controls. second/ retains the initial wrapper's interrupted series. It encountered a local text-decoding error at output-directory reuse; the third/ series completed. Twelve invalid engineering selections rejected. Seven other selector comparisons agreed with main. The stored controls cover 566 timed collector cases, eight observer API checks, four simulated-port scenarios and seven repository tests. No replacement boot or physical run was performed. CHG-073 is separate future work.

CHG-063 and CHG-068 are traced to REQ/VER-002, 005, 008, 016 and 023. POL-002 and EVD-031 cover Pi model scheduling observations. [Trace matrix](../../trace/links.csv) · [Verification plan](../verification-plan.en.md) · [Physical evidence issue](https://github.com/Romanof123/JNOX/issues/34)
