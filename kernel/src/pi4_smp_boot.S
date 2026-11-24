.section .rodata.pi4_smp_tables,"a"
.balign 4096
// 39-bit VA. Map only the spin-slot page, the bounded image window,
// and the PL011/GPIO/GIC pages. Attr0 Normal WBWA ISH; Attr1 Device-nGnRnE.
pi4_root:
    .quad pi4_low_l2 + 3
    .zero 2*8
    .quad pi4_high_l2 + 3
    .zero 508*8
pi4_low_l2:
    .quad pi4_low_l3 + 3
    .zero 511*8
pi4_low_l3:
    .quad 0x0060000000000703
    .zero 127*8
    .set pi4_image_page, 0x80000
    .rept 128
    .quad pi4_image_page + 0x0040000000000703
    .set pi4_image_page, pi4_image_page + 4096
    .endr
    .zero 256*8
pi4_high_l2:
    .zero 497*8
    .quad pi4_uart_l3 + 3
    .zero 10*8
    .quad pi4_gic_l3 + 3
    .zero 3*8
pi4_uart_l3:
    .quad 0x00600000fe200607
    .quad 0x00600000fe201607
    .zero 510*8
pi4_gic_l3:
    .zero 65*8
    .quad 0x00600000ff841607
    .quad 0x00600000ff842607
    .zero 445*8

.section .bss.pi4_smp_boot,"aw",%nobits
.balign 64
.global pi4_entry_el
pi4_entry_el: .skip 16
.global pi4_boot_guard
pi4_boot_guard: .skip 64
.skip 16384
pi4_secondary_top:

.section .text.pi4_smp_boot,"ax"
.balign 4
.global pi4_secondary_entry
.type pi4_secondary_entry,%function
pi4_secondary_entry:
    msr daifset,#15
    mrs x1,mpidr_el1
    ldr x2,=0xff00ffffff
    and x1,x1,x2
    cmp x1,#1
    b.ne .Lhalt
    mrs x18,CurrentEL
    cmp x18,#4
    b.eq .Lsecondary_el1
    cmp x18,#8
    b.ne .Lhalt
    mov x0,#(1<<31)
    msr hcr_el2,x0
    mrs x0,cnthctl_el2
    orr x0,x0,#3
    msr cnthctl_el2,x0
    mov x0,#0x33ff
    msr cptr_el2,x0
    ldr x0,=0x30d00800
    msr sctlr_el1,x0
    adr x0,.Lsecondary_el1
    msr elr_el2,x0
    mov x0,#0x3c5
    msr spsr_el2,x0
    isb
    eret
.Lsecondary_el1:
    msr spsel,#1
    ldr x0,=pi4_secondary_top
    mov sp,x0
    mrs x0,cpacr_el1
    orr x0,x0,#(3<<20)
    msr cpacr_el1,x0
    isb
    bl pi4_smp_translate
    cbz x0,.Lhalt
    ldr x0,=pi4_entry_el
    str x18,[x0,#8]
    b {secondary}
.Lhalt:
    wfe
    b .Lhalt
.size pi4_secondary_entry,.-pi4_secondary_entry

.global pi4_smp_translate
.type pi4_smp_translate,%function
pi4_smp_translate:
    mrs x1,sctlr_el1
    mov x2,#0x1005
    tst x1,x2
    b.ne .Lfailed
    // Cortex-A72 CPUECTLR.SMPEN. The board stub normally sets this.
    mrs x6,S3_1_C15_C2_1
    tbnz x6,#6,.Lcoherent
    orr x6,x6,#0x40
    msr S3_1_C15_C2_1,x6
    isb
    mrs x6,S3_1_C15_C2_1
    tbnz x6,#6,.Lcoherent
    // Cortex-A72 emulation may model CPUECTLR as RAZ/WI. The pinned
    // raspi4b GIC lacks SecurityExtn; hardware GIC-400 has it.
    ldr x6,=0xff841004
    ldr w6,[x6]
    tbnz w6,#10,.Lfailed
.Lcoherent:
    ldr x3,=pi4_root
    ldr x4,=0x80993519
    mov x5,#0xff
    msr ttbr0_el1,x3
    msr tcr_el1,x4
    msr mair_el1,x5
    dsb sy
    isb
    tlbi vmalle1
    ic iallu
    dsb sy
    isb
    orr x1,x1,x2
    msr sctlr_el1,x1
    isb
    mrs x0,sctlr_el1
    and x0,x0,x2
    cmp x0,x2
    b.ne .Lfailed
    mrs x0,ttbr0_el1
    cmp x0,x3
    b.ne .Lfailed
    mrs x0,tcr_el1
    cmp x0,x4
    b.ne .Lfailed
    mrs x0,mair_el1
    cmp x0,x5
    b.ne .Lfailed
    mov x0,#1
    ret
.Lfailed:
    mov x0,#0
    ret
.size pi4_smp_translate,.-pi4_smp_translate
