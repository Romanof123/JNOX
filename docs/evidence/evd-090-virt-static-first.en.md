# EVD-090 · Minimal AArch64 virt static footprint, first verification

The named minimal ELF meets both inclusive REQ-009 engineering limits. The first checker revision requires two layout corrections. This record covers the static portion of VER-009 for #4 and #21 at source `34454f955a30b0d3be0a4dfc73435a344f2dbff9`. [한국어](evd-090-virt-static-first.md) · [Source PR #185](https://github.com/Romanof123/JNOX/pull/185) · [CHG-090 plan](https://github.com/Romanof123/JNOX/blob/a37f339/docs/scheduling-footprint.en.md)

| Bytes | Minimal boot | Ordinary fixed RR | RR minus boot |
|---|---:|---:|---:|
| Code and constants, full RX | 275 | 14,728 | +14,453 |
| RX-to-RW-end RAM reservation | 20,496 | 115,056 | +94,560 |
| RX-to-RW gap | 3,821 | 1,656 | −2,165 |
| RW reservation | 16,400 | 98,672 | +82,272 |
| PT_LOAD file payload | 275 | 15,016 | +14,741 |
| Whole ELF file | 71,128 | 94,096 | +22,968 |

Both RX segments start at `0x40200000`. The minimal RW interval is `[0x40201000, 0x40205010)`. Its span reconstructs as 275 + 3,821 + 16,400 = 20,496 B. Fixed RR has RW `[0x40204000, 0x4021c170)` and 14,728 + 1,656 + 98,672 = 115,056 B. Both images have 0 B internal RX padding and 8 B internal RW padding. The 16,384 B boot stack lies inside each RW interval. The fixed runtime’s 81,920 B of task stacks lies inside its 81,992 B BSS. Remaining minimal headroom is 261,869 B code and 2,076,656 B RAM. Fixed RR has no assigned ceiling.

From a clean detached source, each build used `cargo +1.95.0 build --target aarch64-unknown-none --release --locked -j1` from `kernel/` in a separate target directory. Both selected `JNOX_ARCH=aarch64 JNOX_BOARD=virt`; only RR added `JNOX_PROFILE=fixed JNOX_APP=fixed-rr`. The manifests record the empty feature set, the dispatcher’s boot and ordinary fixed-RR selections, Cortex-A72, release opt-level s and the pinned LLD 22.1.2. GNU readelf 2.38 plus independent file and address byte sets matched all 10 metrics, signed deltas and hashes. The two host suites passed 3 and 4 tests, respectively.

| SHA-256 | Value |
|---|---|
| Minimal ELF | `c90b56c85e939df91710124411204c0024ebc054b3a35f314fce7a96967d06be` |
| Fixed-RR ELF | `c852c3f0c53af512c9e3320d256459fa9eb1615a6c7857b25e8c15dc9fde07ec` |
| Archived evidence | `8810c8505a82e821f5532d7ff171155f083c09cc633d23d8863bbb0648d72102` |

The unchanged linker script produced controls with code and RAM exactly at their limits (exit 0), code one byte over (exit 1), RAM 16 B over, the first possible aligned step (exit 1), and both over (exit 1). An over-budget comparator alone returned 0. A truncated ELF in either role returned 2. A padded fixture with 28 B internal RX padding and 4,045 B between loads charged both in their prescribed domains.

Two independent controls identified first-revision errors in either argument position. A variant linker script left BSS ending at `0x40205029`, while the repository script aligns the BSS end for the 8-byte startup clear. The checker accepted the variant with exit 0. A positive control linked with the unchanged script had ordinary read-only `.rodata` carrying `SHF_GNU_RETAIN`. The extra linker retention flag preserved the section permissions and all measured extents. The checker rejected it with exit 2. The first verdict requires checking both BSS bounds against the linker contract and supporting the retained section.

The archive contains both original ELF files, GNU program and section listings, build selections and logs, checker output, the independent arithmetic, linked controls, first failures and SHA-256 inventory. Runtime peak memory, startup timing and physical-board latency retain separate verification. [Measurement record](evd-090-virt-static-first.json) · [Evidence archive](evd-090-virt-static-first.zip)
