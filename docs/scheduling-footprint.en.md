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
