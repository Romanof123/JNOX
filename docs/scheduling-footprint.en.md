# Scheduling release ELF footprint (#66, #21)

Compare actual linked minimal and extended release artifacts. Use matching target, toolchain, profile, linker script and build settings except for the intended features. Keep the build commands, revision and feature selections with the JSON. Labels and hashes alone cannot prove those settings.

```sh
python tools/scheduling_footprint.py --baseline out/minimal.elf --feature extended=out/extended.elf > footprint.json
```

Repeat --feature NAME=PATH for additional builds. Success emits one JSON object with baseline, named features and signed byte deltas (feature minus baseline). Invalid input exits 2 with a diagnostic on stderr. Same-machine ELF64 little-endian AArch64 or x86-64 executables with ordinary program and named section tables are supported.

| Byte field | Calculation |
|---|---|
| file | Actual file length, including headers, metadata and trailing bytes. SHA-256 covers exactly this file. |
| load_file | Union of PT_LOAD file offset ranges of length p_filesz. |
| load_memory | Union of PT_LOAD virtual address ranges of length p_memsz. |
| load_backed | Union of PT_LOAD virtual address prefixes of length p_filesz. |
| zero_fill | load_memory minus load_backed. A backed mapping takes precedence in overlaps. |
| alloc | Union of nonempty SHF_ALLOC section virtual ranges. Each must lie within the load memory union. |
| padding | load_memory minus alloc, including mapped headers and unnamed gaps. |
| bss | Union of allocated SHT_NOBITS sections named .bss or .bss.*. |
| linked_stack | Union of allocated sections named .stack or .stack.* including NOLOAD reservations. |

All ranges are half-open. Unions count overlapping bytes once within each metric. Metrics in different address spaces or overlapping categories must not be added. Zero-sized ranges contribute zero. Named section counts depend on linker naming. Linked stack measures a reservation. Runtime stack high water, heap use, physical placement, scheduling cost and actual bare-metal loader behavior require separate evidence.

[Independent measurements of 30 release configurations](evidence/scheduling-footprint-review.en.md)

## CHG-090 · Minimal virt engineering acceptance

For #4 and #21, apply the two REQ-009 ceilings to one named linked configuration. Both limits are inclusive. This decision defines the static portion of VER-009 under DES-001 and DES-004. Independent artifact results remain pending. [REQ-009](requirements.en.md) · [VER-009](verification-plan.en.md)

| Role | Exact configuration |
|---|---|
| Minimal, scored | From `kernel/`, use `JNOX_ARCH=aarch64 JNOX_BOARD=virt`, with every other JNOX selector absent. Use no Cargo features. The dispatcher selects boot. |
| Scheduling comparison | Add only `JNOX_PROFILE=fixed JNOX_APP=fixed-rr`. This selects the ordinary Rust fixed-priority round-robin workload. Leave cost, witness, trace, bench features and other selectors absent. Report both absolute counts and signed byte differences against minimal. |

Build both separately with `cargo +1.95.0 build --target aarch64-unknown-none --release --locked -j1`, using the repository Cortex-A72 flags and release optimization without overrides. Preserve revision, exact arguments and environment, compiler/linker identity, ELF SHA-256 and independent program/section listings. The selector belongs in the manifest because ELF geometry alone cannot identify a workload. [Build selection](../kernel/build.rs) · [Target flags](../kernel/.cargo/config.toml)

| Minimal-image quantity | Calculation and ceiling |
|---|---|
| Code and constants | Charge the entire file-backed RX segment, including alignment between text and constants. Require RX file size = RX memory size and **≤262,144 bytes**. |
| Static RAM reservation | Charge the physical interval from RX start through the exclusive end of RW memory, **≤2,097,152 bytes**. This includes RX, the inter-segment alignment gap, initialized data, zero-fill and all padding within RW. Buffers, pools, boot stack and any task stacks in BSS are already charged once. |

Accept only a little-endian AArch64 ELF64 ET_EXEC matching the virt linker script. Require exactly two ordered, nonoverlapping, nonempty PT_LOADs, RX then RW, identity VMA/LMA, valid aligned offsets and bounded sizes. Entry and RX start are 0x40200000. RW begins on a 4 KiB boundary and ends at or below 0x48000000. Account every allocated section in its compatible segment, including the aligned 16,384-byte NOLOAD boot stack. Reject unsupported program headers, layouts or unaccounted allocations. Boot sets SP to the stack top and clears the linked BSS. Record segment extents, intra-segment padding and the inter-segment gap separately so the physical span can be reconstructed from the listings. [Linker script](../kernel/link.ld) · [Startup](../arch/aarch64/start.S)

Use `python tools/virt_footprint.py --minimal MINIMAL.elf --fixed-rr FIXED.elf` for the strict assessment. A conforming minimal layout meets this gate only if both sizes fit. A well-formed over-budget minimal image fails; an unsupported artifact cannot qualify either way. The comparison has no assigned size limit. Report whole ELF file bytes and loadable file payload separately for storage planning. This RAM-loaded virt configuration assigns no nonvolatile storage budget. Firmware, the first 2 MiB reserved for the device tree, MMIO apertures, unallocated guest RAM and host storage lie outside the charged kernel span. Linked reservations do not measure live stack or allocator peaks. Runtime initialization, physical-board latency, other ports and product memory allocation retain their own verification decisions.
