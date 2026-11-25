# Raspberry Pi 4 independent verification

Candidate d905d924a90ea1041821bac78919194852263fe5. Prior candidate 001dda0 failed its first FIFO kernel compilation with duplicate .Lpark. Initial host archive CRLF and missing Git metadata failures are preserved. The corrected cohort used the original ordered C and Rust ELF hashes in apps.sha256. A Linux Git checkout produced clean manifests. Upstream QEMU 11.1.3 raspi4b TCG SHA-256 651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211 ran default reset and clock, four vCPUs, one guest at a time. Firmware boot and physical hardware remain unmeasured.

| First position | Timer rotations | Private C / Rust counts | Exit statuses |
|---|---:|---|---|
| FIFO plain | 0 | 78498 / 70435 | 12 / 77 |
| FIFO traced | 0 | 78498 / 70435 | 12 / 77 |
| RR plain | 16 | 78498 / 70435 | 12 / 77 |
| RR traced | 16 | 78498 / 70435 | 12 / 77 |

CPU0 alone took recorded EL0 exceptions. Each of CPUs 1, 2, 3 ended at model reset park PC 0x310. RR first rotation A to B to A happened before either exit. Independently disassembled workers, saved/restored stack pointer and PC, and IRQ/ERET observations match. CNTFRQ was 62500000. The physical dump covers only live kernel RAM [0x02000000,__stack_top). Each independent TTBR0 page walk resolves the same VA 0x402000 to PA 0x2036000 or 0x244b000, with separate private stack and protected code/data, guard holes and privileged-only devices. Full physical counts and both kernel exit slots were read in all four captures. See inspection.json per attempt for numeric addresses. Scored UART, register monitors, exceptions, argv, and exact images are included.

Virgin virt FIFO and RR with the frozen apps completed 12/77. Existing linked Pi fixed FIFO smoke passed. Both Pi firmware trees passed their pinned checks and raw-image byte comparison. CLI invalid board, policy, count and invalid ELF rejected. Cargo invalid policy, count, and feature rejected. A C source primary and sensor peer exercised separate argument lists, yield and normal exits 17/29. Two 8MiB padded input containers produced a compact 16,816,000-byte raw whose endpoint is 0x1089780 and whose live stack ends at 0x3855000. Its model boot returned 12/77. The ordinary small kernels' ELF PT_LOAD initialized byte ranges were compared against raw bytes with no trailing BSS. A CRLF in the first linked-smoke host wrapper prevented its launch, and the separate Linux command successfully ran its first guest.

The monitor captured terminal state after a fixed four seconds. Raw board reset bypasses firmware execution. Physical power cycling, UART, board timing and product acceptance remain separate.
