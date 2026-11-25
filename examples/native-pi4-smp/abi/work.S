.text
.p2align 2
.global smp_abi_work
.type smp_abi_work,%function
smp_abi_work:
    // x0 role, x1 report, x2 private TLS cell, x3 linked signature.
    // x19 is the only callee-saved GPR touched; q0/q1/q16 are caller-saved.
    sub sp,sp,#32
    mrs x9,tpidr_el0
    mrs x10,fpcr
    mrs x11,fpsr
    stp x19,x9,[sp]
    stp x10,x11,[sp,#16]
    str x9,[x1,#64]
    mrs x9,tpidrro_el0
    str x9,[x1,#80]
    str x0,[x1,#8]
    mov x19,x3
    str x19,[x2]
    msr tpidr_el0,x2
    add x5,x0,#1
    lsl x9,x5,#22
    msr fpcr,x9
    eor x9,x0,#1
    lsl x9,x9,#27
    msr fpsr,x9
    lsl x9,x5,#8
    add x9,x9,#1
    mov v16.d[0],x9
    add x9,x9,#1
    mov v16.d[1],x9
    mov x4,#0
    mov x6,#0
    movi v1.2d,#0
    mov x7,#0x2000000
    mov x9,#1
    str x9,[x1]
.global smp_abi_loop
smp_abi_loop:
    add x6,x6,x5
    fmov d0,x5
    add d1,d1,d0
    add x5,x5,#2
    add x4,x4,#1
    cmp x4,x7
    b.ne smp_abi_loop
.global smp_abi_loop_end
smp_abi_loop_end:
    stp x4,x6,[x1,#16]
    str d1,[x1,#32]
    str x19,[x1,#40]
    str q16,[x1,#48]
    mrs x9,tpidr_el0
    str x9,[x1,#72]
    ldr x10,[x9]
    str x10,[x1,#112]
    mrs x9,tpidrro_el0
    mrs x10,fpcr
    stp x9,x10,[x1,#88]
    mrs x9,fpsr
    str x9,[x1,#104]
    ldp x10,x11,[sp,#16]
    msr fpcr,x10
    msr fpsr,x11
    ldp x19,x9,[sp],#32
    msr tpidr_el0,x9
    mrs x10,tpidr_el0
    str x10,[x1,#120]
    mov x9,#2
    str x9,[x1]
.global smp_abi_restored
smp_abi_restored:
    ret
.size smp_abi_work,.-smp_abi_work
.section .note.GNU-stack,"",%progbits
