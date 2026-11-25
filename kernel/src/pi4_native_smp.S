.section .bss.pi4_native_smp_stacks,"aw",%nobits
.balign 4096
.global pi4_native_smp_stacks
pi4_native_smp_stacks:
    // Each unit begins with a 4 KiB page omitted from every process root.
    .skip 4 * 0x5000
.section .bss.pi4_native_smp_stamps,"aw",%nobits
.balign 64
.global pi4_native_smp_stamps
pi4_native_smp_stamps:
    .skip 2 * 64

.section .text.pi4_native_smp,"ax",%progbits
.balign 4
.global pi4_native_smp_secondary
pi4_native_smp_secondary:
    msr daifset,#15
    mrs x0,mpidr_el1
    ldr x1,=0xff00ffffff
    and x0,x0,x1
    cmp x0,#1
    b.ne .Lpi4_native_smp_park
    mrs x0,CurrentEL
    cmp x0,#4
    b.eq .Lpi4_native_smp_check_el1
    cmp x0,#8
    b.ne .Lpi4_native_smp_park
    mrs x0,sctlr_el2
    tst x0,#1
    b.ne .Lpi4_native_smp_park
    tst x0,#4
    b.ne .Lpi4_native_smp_park
    mov x0,#(1<<31)
    msr hcr_el2,x0
    mrs x0,cnthctl_el2
    orr x0,x0,#3
    msr cnthctl_el2,x0
    mov x0,#0x33ff
    msr cptr_el2,x0
    ldr x0,=0x30d00800
    msr sctlr_el1,x0
    adr x0,.Lpi4_native_smp_check_el1
    msr elr_el2,x0
    mov x0,#0x3c5
    msr spsr_el2,x0
    isb
    eret
.Lpi4_native_smp_check_el1:
    mrs x0,sctlr_el1
    tst x0,#1
    b.ne .Lpi4_native_smp_park
    tst x0,#4
    b.ne .Lpi4_native_smp_park
    mrs x0,S3_1_C15_C2_1
    orr x0,x0,#64
    msr S3_1_C15_C2_1,x0
    isb
    msr spsel,#1
    ldr x0,=pi4_native_smp_stacks + 0xf000
    mov sp,x0
    mrs x0,cpacr_el1
    orr x0,x0,#(3<<20)
    msr cpacr_el1,x0
    ic iallu
    dsb sy
    isb
    mov x0,#1
    b native_partitioned_entry
.Lpi4_native_smp_park:
    msr daifset,#15
1:  wfe
    b 1b

.macro pi4_native_smp_vector target
    .balign 128
    b \target
.endm
.balign 2048
.global pi4_native_smp_vectors
pi4_native_smp_vectors:
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_idle_irq
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_svc
    pi4_native_smp_vector .Lpi4_native_smp_irq
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
    pi4_native_smp_vector .Lpi4_native_smp_park
.balign 128

// x0 = CPU, x1 = selected frame or zero. IRQ masked until architectural return.
.global pi4_native_smp_enter
pi4_native_smp_enter:
    msr daifset,#2
    ldr x2,=pi4_native_smp_stacks + 0x5000
    mov x3,#0xa000
    madd x2,x0,x3,x2
    mov sp,x2
    cbz x1,.Lpi4_native_smp_idle
    mov x30,x1
    ldp x2,x3,[x30,#248]
    msr sp_el0,x2
    msr elr_el1,x3
    ldp x2,x3,[x30,#264]
    msr spsr_el1,x2
    msr tpidr_el0,x3
    ldp x2,x3,[x30,#280]
    msr tpidrro_el0,x2
    msr fpcr,x3
    ldr x2,[x30,#296]
    msr fpsr,x2
    .irp n,0,2,4,6,8,10,12,14,16,18,20,22,24,26,28,30
    ldr q\n,[x30,#(304+16*\n)]
    .endr
    .irp n,1,3,5,7,9,11,13,15,17,19,21,23,25,27,29,31
    ldr q\n,[x30,#(304+16*\n)]
    .endr
    ldr x16,=pi4_native_smp_stamps
    add x16,x16,x0,lsl #6
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,18,19,20,21,22,23,24,25,26,27,28,29
    ldr x\n,[x30,#(8*\n)]
    .endr
    isb
    mrs x17,cntpct_el0
    stlr x17,[x16]
    ldp x16,x17,[x30,#128]
    ldr x30,[x30,#240]
    eret
.Lpi4_native_smp_idle:
    add x2,x2,#5,lsl #12
    mov sp,x2
    adr x1,.Lpi4_native_smp_idle_landed
    msr elr_el1,x1
    mov x1,#0x3c5
    msr spsr_el1,x1
    eret
.Lpi4_native_smp_idle_landed:
    ldr x1,=pi4_native_smp_stamps + 8
    add x1,x1,x0,lsl #6
    isb
    mrs x2,cntpct_el0
    stlr x2,[x1]
    msr daifclr,#2
    isb
1:  wfi
    b 1b

.macro pi4_native_smp_save_user
    sub sp,sp,#832
    stp x0,x1,[sp]
    isb
    mrs x1,cntpct_el0
    str x1,[sp,#816]
    .irp n,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30
    str x\n,[sp,#(8*\n)]
    .endr
    mrs x2,sp_el0
    mrs x3,elr_el1
    stp x2,x3,[sp,#248]
    mrs x2,spsr_el1
    mrs x3,tpidr_el0
    stp x2,x3,[sp,#264]
    mrs x2,tpidrro_el0
    mrs x3,fpcr
    stp x2,x3,[sp,#280]
    mrs x2,fpsr
    str x2,[sp,#296]
    .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
    str q\n,[sp,#(304+16*\n)]
    .endr
    mrs x0,mpidr_el1
    and x0,x0,#255
    mov x1,sp
    ldr x2,[sp,#816]
.endm
.Lpi4_native_smp_svc:
    pi4_native_smp_save_user
    mrs x3,esr_el1
    bl native_partitioned_sync
    b .Lpi4_native_smp_selected
.Lpi4_native_smp_irq:
    pi4_native_smp_save_user
    b .Lpi4_native_smp_ack
.Lpi4_native_smp_idle_irq:
    isb
    mrs x2,cntpct_el0
    mrs x0,mpidr_el1
    and x0,x0,#255
    ldr x1,=pi4_native_smp_stacks + 0x5000
    mov x3,#0xa000
    madd x1,x0,x3,x1
    mov sp,x1
    mov x1,xzr
.Lpi4_native_smp_ack:
    ldr x3,=0xff84200c
    ldr w3,[x3]
    bl native_partitioned_irq
.Lpi4_native_smp_selected:
    mov x1,x0
    mrs x0,mpidr_el1
    and x0,x0,#255
    b pi4_native_smp_enter
.ltorg
.section .pi4_native_secondary_shim,"ax",%progbits
.balign 4
.global pi4_native_smp_trampoline
pi4_native_smp_trampoline:
    msr daifset,#15
    mrs x0,mpidr_el1
    ldr x1,=0xff00ffffff
    and x0,x0,x1
    cmp x0,#1
    b.ne 2f
    mrs x0,CurrentEL
    cmp x0,#8
    b.eq 1f
    cmp x0,#4
    b.ne 2f
1:  ic iallu
    dsb sy
    isb
    ldr x0,=pi4_native_smp_secondary
    br x0
2:  wfe
    b 2b
.ltorg
