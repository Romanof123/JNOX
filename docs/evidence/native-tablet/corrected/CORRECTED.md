# Corrected native tablet evidence for JNOX #451

Both independent AArch64 guest replays pass on source `560bcb768e6287da8732c2e81a9a4ece166ba9f6`, kernel SHA-256 `5eb46bf0950905ea06a175f15db7cf39a7e03d40ce6f39a5eeef7dd3ad59e2e6`.

| Change of pointer focus | Accepted send records, first boot | Accepted send records, second boot |
| --- | --- | --- |
| Pulse leave → Tiles enter as the tablet moves | 53 → 54 | 54 → 55 |
| Tiles leave → Pulse enter in the overlap | 59 → 60 | 60 → 61 |
| Pulse leave → Tiles enter on restack | 64 → 65 | 65 → 66 |

These are complete wire-message positions reconstructed from the compositor's socket-send journal. The previous kernel wrote the overlap pair in the opposite order (new enter 59, old leave 60). We reversed the actual socket records on copies of both corrected captures. Each of the six reversed pairs failed the reader. [Wayland pointer specification](https://wayland.freedesktop.org/docs/html/apa.html)

**Input and application receipts.** The first six QMP submissions produce 14 split-ring completions per boot. Used IDs resolve through writable descriptors into actual eight-byte evdev events, with the expected button edges, values and SYN_REPORT boundaries. All ten QMP packets match the 24 terminal kernel event-history words. The final four packets have terminal event history and application receipts. Individual DMA snapshots exist for the first six packets. The completed clicks go to Pulse at client-local (24,30) and Tiles at (118,66). In both processes, the results of the tablet probe and poll are checked against -1 before the denial witness advances from 3 to 5. The observer independently locates those counters and buffer mappings through each process’s ELF and page tables. Every pixel in all five QEMU framebuffer snapshots per attached boot matches the visible regions of the appropriate client buffers.

**Lifecycle.** The pointer follows the topmost overlapping surface. When the focused Tiles process exits, Pulse regains focus and receives motion through (114,76). Both boots report four expected UART exits. The hardware STATUS falls from 15 to 0, both QueueReady registers read zero when selected separately and the archived DMA allocation is fully zero. The device-absent run exposes only the keyboard to both clients. Both ordinary keyboard boots produce the correct focused key events and all six corresponding frames, and the headless program observes -38 on both tablet operations and display init before exiting 47.

**Explicit destruction and partial writes.** A separate independent test ran the packaged Linux host compositor on two local AF_UNIX connections. Pulse destroyed its xdg_toplevel, xdg_surface and wl_surface in protocol order, received the surface ID deletion, and completed another sync with the same connection still open. Tiles got stationary enter at (8,8) and subsequent motion at (13,12). Its full client-side send/receive byte transcript is in `results/host-destroy-first.json`. The target guest exercised abrupt process disconnect. The recorded host unit test advanced 15 of a leave message's 16 bytes and kept the later peer blocked until the final byte. Sustained guest socket backpressure was not exercised.

**Scope of hardware findings.** MMIO identity was read at 0x0a003e00, device 18, version 2. The selected Y ABS_INFO readback was 0–32767, and the driver receipt recorded the same range for X. Owner exit/reset, clean absence, real input, cross-client delivery and ordinary regressions are observed. Forced malformed DMA, lost reports, out-of-range hardware axes, per-call maximum work, and guest owner reacquisition are outside the observed runs. The host capability-loss fixture used its test input socket.

## Reproduce

Extract the archive into an empty directory. Python 3 and GNU AArch64 objdump are sufficient for the recorded runs. The optional live host check uses an x86-64 Linux environment and local Unix sockets.

```sh
for run in attached-one attached-two; do
  python3 -B reader.py package "$run" --report "$run.replay.json" || exit 1
  python3 -B controls.py package "$run" "$run" --corrected --report "$run.controls.json" || exit 1
done
python3 -B reader.py package absent --absent --report absent.replay.json
python3 -B profiles.py . profiles.replay.json
chmod u+x wayland-host
python3 -B host_destroy.py ./wayland-host surface-destroy.replay.json
```

Each attached boot rejects fifteen controlled mutations. Ten target QMP, descriptor, payload, journal, private receipt, image and device-status data. Two set each retired queue's Ready bit. Three reverse one pair of actual cross-connection sends. The reader ignores the producer’s predecoded app and wire summaries.
