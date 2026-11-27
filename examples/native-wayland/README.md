# Native Wayland clients

Pulse and Tiles run as separate AArch64 EL0 processes. Both connect to `/wayland-0` over AF_UNIX streams through socket broker slot 3. They bind version 1 of `wl_compositor`, `wl_shm`, `xdg_wm_base` and `wl_seat`. The client encodes native-endian Wayland messages directly. Its JNOX dependencies are the local socket transport, native shared backing, clock and display ownership check.

Build the client ELF from the repository root in the native Linux build environment. Package it in slots 1 and 2 with arguments `a` and `b`. Slot 0 runs the compositor and slot 3 runs the ordinary socket broker. The package uses the native display and shared-FD features.

```sh
sh sdk/native-aarch64/build.sh --local-socket examples/native-wayland/client.c "$out/client.elf"
```

## Wire and buffer lifecycle

The client-side opcode table derives from the positions of requests and events within their upstream interface definitions. Requests and events have independent numbering. Versions above 1 are excluded by the client's binding choice. The checked source files are Wayland 1.23.1 `protocol/wayland.xml`, SHA-256 `0c371e9c31f8178008a7ddecf431cbe12ec4b29ef7714803ecd3e20af925e7ed`, and wayland-protocols 1.37 stable `xdg-shell.xml`, SHA-256 `a43961371b6f74af4861b3ba71fc0159d8564c33ec19637581b5daa689b103be`. [Core XML](https://raw.githubusercontent.com/wayland-mirror/wayland/1.23.1/protocol/wayland.xml) · [Stable xdg-shell XML](https://raw.githubusercontent.com/wayland-mirror/wayland-protocols/1.37/stable/xdg-shell/xdg-shell.xml) · [Client opcode table](protocol.h)

Each connection starts with display object 1. A registry round trip discovers the actual global names before binding. The client receives a keyboard capability, requests a keyboard, takes the XKB-v1 keymap FD from the ordered ancillary queue, maps it read-only and hashes the supplied bytes. Keyboard input uses Linux evdev keycodes. Each client checks that a key event targets its current keyboard focus.

Each client creates a surface, assigns an xdg-toplevel role, sets its title and app ID, and makes an empty initial commit. After `xdg_toplevel.configure` and `xdg_surface.configure`, it acknowledges the serial. Its pool contains one 128 × 96 XRGB8888 buffer with stride 512. The client maps the original object writable, passes a read-only duplicate with `wl_shm.create_pool` using `SCM_RIGHTS`, then closes both local numeric FDs. The writable mapping remains live. Each redraw writes those mapped pixels, attaches the buffer, damages its full extent, requests a frame callback and commits. The client waits for buffer release before writing again and for callback completion plus `wl_display.delete_id` before reusing callback ID 14.

The runner sends **x**, **n**, **x**, **k**, **x**, **q** on PL011. The compositor sends focused keyboard events for **x** and **k**. Each **x** press changes only the recipient’s mapped image. Tiles handles **k** by submitting one last buffer and exiting before it receives the release or callback. Its socket, protocol objects and client mapping are live at exit. Pulse accepts another **x** and closes its objects in response to `xdg_toplevel.close`, waits for a final sync, unmaps and closes its socket. The expected client exits are 71 for Pulse and 72 for Tiles. Failure returns 100 and preserves the first failing C source line.

## Process-private observations

Resolve `wayland_client_witness[32]` through the corresponding client's address-space root. Entries are unsigned little-endian 64-bit words.

| Words | Meaning |
| --- | --- |
| 0–4 | Role (1 Pulse, 2 Tiles), stage, first failing source line, errno at failure, and fatal display error code. Stages 1–4 cover connection, binds, initial configure and buffer commits. Stage 5 marks Tiles’ live-resource exit and 6 marks orderly teardown. |
| 5–10 | Four-global discovery mask, advertised XRGB8888 flag, keymap byte count and FNV-1a byte hash, configure count, and last acknowledged serial. |
| 11–16 | Keyboard enter and leave counts, focus flag, accepted **x** and **k** press counts, and last press serial. |
| 17–23 | Buffer commits, releases, frame callbacks completed, last pixel hash, producer mapping VA, original pool FD, and read-only FD sent. |
| 24–31 | Buffer busy at Tiles’ exit, close event seen, frame ID deletions, key release count, number of denied direct device operations, last callback timestamp, last pressed keycode, and final sync done. |

The pixel hash starts at 2166136261 and advances per row-major 32-bit pixel as `(hash ^ pixel) * 16777619` modulo 2³². Pulse displays its own waveform and Tiles its own tile pattern. The compositor receives a generic shared image and the application metadata.
