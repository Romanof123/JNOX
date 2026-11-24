# M1 static time partition dispatch

[한국어](m1-partition.md) · English

CHG-012 · POL-006 · IMP-010 · EVD-010 · #33

Two four-tick major frames exercise the repeating table. Ticks are logical times. A owns offsets 0–1, B owns offset 2, and offset 3 is explicitly idle.

| Tick | Required execution |
| --- | --- |
| 0 | A enters and works without yielding. |
| 2 | The timer preempts A. B is blocked, so idle enters. |
| 3 | B becomes ready during explicit idle. The interrupted idle context continues in this frame. |
| 4 | The next frame opens. A resumes its interrupted work. |
| 6 | The timer again preempts non-yielding A. Ready B enters its assigned window. |
| 7 | Explicit idle resumes. |
| 8 | Stop the timer and publish the terminal verdict. |

Run `python3 tools/qemu_boot.py --profile partition --kernel "$ELF"` against the captured image. Record the first black-box verdict, including failure, from one fresh guest. The observer requires the following UART lines in order, accepts LF or CRLF, checks 0.1 seconds of silence, and stops QEMU.

```text
JNOX M1 BOOT
JNOX M1 READY
PART A FIRST
PART IDLE FIRST
PART B READY
PART A RESUMED
PART B FIRST
PART IDLE RESUMED
PART STOP
PART PASS
```

Retain the revision, ELF hash, launch arguments, raw UART and exception trace from the same run. Map the six timer IRQs to the linked ELF and inspect interrupted PCs, exception returns and dispatch sites. Read the matching source and disassembly to establish which timer deadlines represent ticks 2, 3, 4, 6, 7 and 8, how B becomes eligible, and where A and idle continue. UART establishes marker order. IRQ counts and PCs alone cannot identify logical time or task readiness. This contract covers the specified single-core QEMU workload. Board latency, memory isolation and behavior under arbitrary partition tables require separate evidence.

REQ-023/002/005 → DES-012/003 → IMP-010 → VER-023/002/005. Host tests compare bounded valid window tables and every readiness mask against an independent oracle. Record host results separately from guest exceptions.

```sh
(cd kernel && cargo build --release --locked --features partition-bench -j1)
```

[EVD-010](evidence/evd-010.en.md)

CHG-046 · [Two virtual window tables and recovery criteria for ordinary Rust tasks](pol-006-rust-partition-plan.en.md) · [#99](https://github.com/Romanof123/JNOX/issues/99)

POL-006 / IMP-044. Both first ordinary runs failed the 1 ms criterion. Q skipped an eligible B window. Live masking and complete release qualification remain open. [EVD-044](evidence/evd-044.en.md)
