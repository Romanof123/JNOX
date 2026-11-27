# Independent tablet acceptance, JNOX #451

Source `1feb5b1f4773486dd6c92edae74af740e9fcbf22` **fails the Wayland focus send order**. When moving back onto Pulse, the compositor's global send journal records Pulse enter at 59, Tiles leave at 60 and Pulse motion at 61. It assigned leave serial 13 and enter serial 14. Across the reverse switch, Pulse leave at 52 precedes Tiles enter at 53. Each peer gets the correct coordinates, button events and palette. The protocol requires the old surface’s leave notification to be sent first. [Wayland pointer specification](https://wayland.freedesktop.org/docs/html/apa.html)

Unzip the original evidence archive to an empty directory. From that directory, run these passive checks. The first command exits 1 on the recorded regression. `profiles.py` uses GNU AArch64 objdump. The other readers use Python 3.8+ standard library.

```sh
python3 -B reader.py package queue-reset-control --report replay.json
python3 -B reader.py package absence-control --absent --report absence.json
python3 -B controls.py package queue-reset-control queue-reset-control --report controls.json
python3 -B profiles.py . profiles.json
```

The kernel SHA-256 is `b32a72d7ac8db78a20499948c645b76f5901997a9da352e5dba5c97ed9559ee2`. The reader resolves symbols from the embedded ELF files, walks separate 39-bit AArch64 EL0 page tables, reassembles socket chunks into Wayland messages and checks both client receipts. It recomposes the 320 × 240 screen from pixels in the two client address spaces and checks it against the external P6 screendump. For the two clicks it follows each device-written used ID to the descriptor’s actual physical buffer, reads all eight event bytes, checks replenishment and compares the events and SYN boundaries against six recorded QMP argument packets and the kernel receipt. Those six packets produced 14 completions. Additional motion during overlap and disconnect brings terminal counters to 24 completions and 10 reports; individual intermediate DMA snapshots for those four later reports are unavailable.

| Scope | Result on this kernel |
| --- | --- |
| Emulated input | Physical MMIO at 0x0a003e00 has magic 0x74726976, version 2, device 18, STATUS 15 and selected QueueReady 1. Driver reports VERSION_1 negotiated, IRQ 79 masked and both axes 0–32767. HMP directly reads the selected Y ABS_INFO range. |
| Clicks and wire | Pulse receives (24,30) and Tiles (118,66) in local pixels, each with one BTN_LEFT press and release. Both private buffers change and the QEMU image matches those buffers. |
| Distinct permissions | Both processes have independent receipt physical addresses. In their linked binaries the witness increments by two only after tablet probe and poll each return -1. Both private counts reach five. |
| Overlap | Pulse takes overlap at (104,66), then receives motion at (109,71). Older-client leave is sent after the new-client enter. |
| Disconnection | Tiles holds focus after restacking, its connection closes and Pulse gets a new enter then motion to (114,76). No separate live-client explicit focused-surface destroy request was stimulated by this phase. |
| Owner retirement | Four UART exits have status 0,71,72,0. Physical STATUS is zero, ISR is zero, both queue selectors independently read Ready=0, the owner is retired and both 4096-byte DMA pages are zero. |
| Absence and regressions | Same opt-in image without a tablet advertises keyboard only to both clients. Two ordinary keyboard boots replay six framebuffer stages each and deliver x twice to Pulse and x/k to Tiles. The distinct headless package returns -38 for tablet probe, poll and display init before UART exit 47. |
| Controls | The replay rejects all ten corruption controls, covering QMP input, used length/cursor, DMA direction and contents, replenishment, client receipt, duplicate wire serial, a single framebuffer channel and reset status. |

The QMP injector retained argument packets but no individual host acknowledgments. The actual device completions provide the independent receipt. The direct device capture retained the selected Y configuration; the X range is present in the driver record. The existing captures provide no runtime injection of malformed or dropped packets, out-of-range axes or configuration changes. Quarantine, maximum work per poll and owner reacquisition require different evidence. The issue covers emulated AArch64 virt hardware. [Virtio 1.2](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html) · [QEMU QMP input](https://www.qemu.org/docs/master/interop/qemu-qmp-ref.html)

The earlier `8ff114ca6f5b18dbd3766004ea012395f77a40de` run already showed the two correct clicks. It lacks the two non-owner checks, complete guest exits, terminal hardware registers and overlap evidence. Its independent result remains in the verifier workspace as `first-build-8ff114ca.json`.
