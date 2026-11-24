.section .rodata.smp_tables, "a"
.balign 4096
smp_l1:
    .quad smp_l2_devices + 3
    .quad smp_l2_ram + 3
    .zero 510 * 8
smp_l2_devices:
    .zero 64 * 8
    .quad smp_gic_pages + 3
    .zero 7 * 8
    .quad smp_uart_pages + 3
    .zero 439 * 8
smp_l2_ram:
    .quad 0
    // 2 MiB at the ELF load address. EL1 RWX, EL0 inaccessible, AF=1,
    // Normal WBWA (Attr0), inner shareable. A fixed bench map with no user
    // processes. Check the linked end before enabling this translation.
    .quad 0x0040000040200701
    .zero 510 * 8
smp_gic_pages:
    // 4 KiB Device-nGnRnE (Attr1), AF=1, outer shareable, PXN and UXN.
    .quad 0x0060000008000607
    .zero 15 * 8
    .quad 0x0060000008010607
    .zero 495 * 8
smp_uart_pages:
    .quad 0x0060000009000607
    .zero 511 * 8

.section .bss.smp_secondary_stack, "aw", %nobits
.balign 16
    .skip 16384
smp_secondary_stack_top:

.section .text.smp_boot, "ax"
.balign 4
.global smp_enable_mmu
.type smp_enable_mmu, %function
smp_enable_mmu:
    // MMU and caches are cold at reset/PSCI CPU_ON on this QEMU target.
    // Refuse live translation or cache state: this is not a firmware takeover.
    mrs x1, sctlr_el1
    mov x2, #0x1005
    tst x1, x2
    b.ne .Linvalid
    ldr x3, =__stack_top
    mov x4, #0x40400000
    cmp x3, x4
    b.hi .Linvalid
    ldr x3, =smp_l1
    msr ttbr0_el1, x3
    ldr x4, =0x80993519
    // 39-bit TTBR0, 4 KiB granule, 32-bit PA, WBWA shareable walks;
    // TTBR1 walks disabled. All ELF code and stack addresses stay identical.
    msr tcr_el1, x4
    mov x5, #0xff
    msr mair_el1, x5
    dsb sy
    isb
    tlbi vmalle1
    ic iallu
    dsb sy
    isb
    orr x1, x1, x2
    msr sctlr_el1, x1
    isb
    mrs x1, sctlr_el1
    and x1, x1, x2
    cmp x1, x2
    b.ne .Linvalid
    mrs x1, ttbr0_el1
    cmp x1, x3
    b.ne .Linvalid
    mrs x1, tcr_el1
    cmp x1, x4
    b.ne .Linvalid
    mrs x1, mair_el1
    cmp x1, x5
    b.ne .Linvalid
    mov x0, #1
    ret
.Linvalid:
    mov x0, #0
    ret
.size smp_enable_mmu, . - smp_enable_mmu

.global smp_secondary_entry
.type smp_secondary_entry, %function
smp_secondary_entry:
    msr daifset, #15
    cmp x0, #{token}
    b.ne .Lbad_entry
    mrs x1, CurrentEL
    cmp x1, #4
    b.ne .Lbad_entry
    mrs x1, mpidr_el1
    ldr x2, =0xff00ffffff
    and x1, x1, x2
    cmp x1, #1
    b.ne .Lbad_entry
    msr spsel, #1
    ldr x1, =smp_secondary_stack_top
    mov sp, x1
    mrs x1, cpacr_el1
    orr x1, x1, #(3 << 20)
    msr cpacr_el1, x1
    isb
    bl smp_enable_mmu
    cbz x0, .Lbad_entry
    b {secondary}
.Lbad_entry:
    // Before a verified MMU/stack handoff CPU1 may not publish or print.
    // CPU0 diagnoses this path via its bounded rendezvous.
    wfe
    b .Lbad_entry
.size smp_secondary_entry, . - smp_secondary_entry
