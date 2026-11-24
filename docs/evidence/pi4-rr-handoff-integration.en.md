# Pi 4 RR handoff on integrated AArch64 source

Four first cold QEMU raspi4b boots exercised f1e2a730614f9313e711b89dcc5a1a9f0b96e760. Ordinary RR completed in O1 and O2. Engineering E1 and E2 recorded HIGH resuming and both peer operations. A released in generation 5, but B’s acquire bracket crossed generations 4–6. Both engineering boots returned 2. Single-generation handoff and later loop progress remain unconfirmed.

| Position | Result | Matched payload | Release / acquire ticks |
|---|---:|---:|---:|
| O1 | 0 | — | — |
| E1 | 2 | 1,958,498 | 631 / 19,896 |
| E2 | 2 | 1,744,682 | 1,121 / 30,303 |
| O2 | 0 | — | — |

The default ELF and Pi 4 directed-wake ELF each matched an independent build of main 17cf31bd02903ea874c8e23b09f342bcebc7ec8b. The SMP ELF also matched the retained #156 ELF byte for byte (SHA-256 17922086bf52386a5868b1c8eaa6efd96b644d527146edff6f51025b955493f6). The four guests exercised RR. Ordinary RR text matched main; two raw-image bytes differed in Rust source line numbers. Relative to v1 engineering, text grew by 452 bytes, read-only data by one and BSS by 176. Original #146 FIFO/RR replays scored 0/0 and integrated v1 scored 0/2. Six malformed capture controls and seven invalid build selectors were rejected. An initial incomplete source archive stopped compilation before any guest. All four positions used the complete pinned source.

[Capture archive](pi4-rr-handoff-integration.zip) · [Digests](pi4-rr-handoff-integration.json)
