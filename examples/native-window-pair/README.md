# Two local windows on JNOX

Two native Rust clients supply pixels for overlapping Pulse and Tiles windows. A third process owns the 320 × 240 ramfb and PL011 input. Press **x** to change the focused window’s color, **n** to bring the other window to the front, and **q** to quit. A light border marks focus. The three processes exchange fixed-size drawing and input records over AF_UNIX streams through the ordinary Rust socket broker.

| Slot | Process | Argument | FIFO priority |
| --- | --- | --- | --- |
| 0 | Display owner and compositor | None | 7 |
| 1 | Pulse client | `a` | 5 |
| 2 | Tiles client | `b` | 4 |
| 3 | Local socket broker | `7` (client slots 0, 1, 2) | 1 |

Slots 0–2 execute the same ELF in separate address spaces. Both clients call display initialization, row submission and input polling directly and require `-EPERM` from each operation. Their ordinary socket connections to `/windows` carry the drawing records. The owner identifies the two declared roles from the initial messages and rejects duplicate roles.

## Build and run the capture

From the root of a clean, committed checkout in the Linux build environment, run the following commands with the native AArch64 SDK, QEMU and Python 3 available.

```sh
out=$(mktemp -d)
export CARGO_TARGET_DIR="$out/target"
sh sdk/native-aarch64/build-rust.sh --local-socket \
    examples/native-window-pair/Cargo.toml "$out/window.elf"
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml "$out/socket.elf"
python3 tools/build_native_el0.py --board virt --policy fifo --out "$out/package" \
    --elf "$out/window.elf" --priority 7 \
    --peer-elf "$out/window.elf" --peer-arg a --peer-priority 5 \
    --peer-elf "$out/window.elf" --peer2-arg b --peer-priority 4 \
    --peer-elf "$out/socket.elf" --peer3-arg 7 --peer-priority 1 \
    --display-service 0 --socket-service 3 \
    --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3
python3 tools/verify_native_window_pair.py --package "$out/package" --out "$out/capture"
```

The verifier captures four frames from QEMU. Its PL011 input sequence is **x**, **n**, **x**, **q**. Pulse starts in front. Its first key changes its body to green while Tiles stays plum. Bringing Tiles forward changes the overlap immediately, and the second **x** changes Tiles to copper. Pulse stays green behind it.

For manual use with a graphical QEMU installation, launch the same package and type the lowercase keys in the serial terminal.

```sh
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -m 128M \
    -device ramfb -serial stdio -monitor none -kernel "$out/package/kernel.elf"
```

The kernel prints the process exits and parks after quit. In the capture sequence, the expected exits in slot order are **70, 71, 72, 0**. Pulse returns 71 only after processing one action with serial 1. Tiles returns 72 only after processing one action with serial 2. A client with a different valid input history returns 0 after the quit handshake.

## Drawing and input

Each 38-byte drawing record supplies a client identity, per-client action count, global input serial, two RGB colors, a local origin and a 16 × 12 one-bit drawing. Each motif pixel covers a 4 × 4 square. The owner validates that the entire drawing fits inside the client area before using it. It draws each scanline back to front and submits one full XRGB row through the display SDK.

Only the focused stream receives an **x** message. Its six bytes identify the operation, recipient, per-client count and global serial. The client checks its identity, the next count and an increasing serial, updates its color, and returns a new drawing with those counters. The owner validates that response before presenting it. On **q**, both clients independently acknowledge their final count and last serial before closing. Both directions handle partial stream transfers.

This local drawing protocol is a precursor to the resident Wayland work. Wayland objects, shared-memory buffer release, compositor restart and physical device qualification have separate acceptance criteria. [Window system plan](../../docs/window-system-plan.en.md)
