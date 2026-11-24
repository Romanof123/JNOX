# Static footprint of 30 scheduling release configurations

[한국어](scheduling-footprint-review.md)

The minimal boot ELF occupied 67,200 bytes on disk and mapped 16,667 bytes. Across 30 configurations, SMP had the largest mapped extent (259,814 bytes), while mutex had the largest named BSS (197,928 bytes). These are measurements of the linked release images for the scheduling coverage tracked in #21. [Footprint issue #66](https://github.com/Romanof123/JNOX/issues/66) · [Scheduling coverage #21](https://github.com/Romanof123/JNOX/issues/21)

| Configuration | ELF file (B) | PT_LOAD mapped (B) | Mapped Δ vs boot (B) | BSS (B) | Linked stack (B) |
|---|---:|---:|---:|---:|---:|
| boot | 67,200 | 16,667 | +0 | 8 | 16,384 |
| scheduler | 85,280 | 61,030 | +44,363 | 32,792 | 16,384 |
| cbs-wake | 89,640 | 85,705 | +69,038 | 55,688 | 16,384 |
| periodic-overrun | 89,032 | 162,576 | +145,909 | 133,160 | 16,384 |
| priority-resume | 87,384 | 182,381 | +165,714 | 152,256 | 16,384 |
| mutex | 99,856 | 233,584 | +216,917 | 197,928 | 16,384 |
| smp | 125,776 | 259,814 | +243,147 | 197,792 | 16,384 |
| all | 87,728 | 79,456 | +62,789 | 49,240 | 16,384 |

PT_LOAD mapped bytes count the union of virtual address ranges. BSS and linked stack sections already occupy part of that union. ELF file length measures on-disk bytes. The stack column measures named reservations at link time. Runtime stack high-water marks, heap peaks, physical placement and scheduling latency remain unmeasured.

The independent verifier used GNU AArch64 readelf section and segment listings and a separate byte-set calculation. It matched all nine metrics, image hashes and feature deltas against the CLI. Two invalid inputs exited 2. An overlapping mapping plus an empty mapping produced 16,672 mapped bytes, including 16,384 zero-filled bytes. Four host tests passed. The JSON includes every feature selection, build command, image hash and metric, with the original listings, verifier and test output in its ZIP archive.

Rust 1.95.0 · `aarch64-unknown-none` · `cargo build --release --locked`. [Full measurements and archived evidence](scheduling-footprint-review.json) · [Metric definitions](../scheduling-footprint.en.md) · [Measurement tool](https://github.com/Romanof123/JNOX/blob/6eb786ab4aa8ac10849c5d0073f279a0e8b26feb/tools/scheduling_footprint.py) · [Tested kernel manifest](https://github.com/Romanof123/JNOX/blob/93c64c853c3d1bf3f6286da59d83d7dc8394f7b9/kernel/Cargo.toml)
