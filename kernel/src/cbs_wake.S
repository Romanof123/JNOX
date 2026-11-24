.macro wake_entry name, tag
.global \name
.type \name, %function
\name:
    sub sp, sp, #16
    mov x19, #\tag
    mov x20, #1
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    msr daifclr, #2
.endm

wake_entry cbs_wake_a, 0xa19
    mov x21, #0
.Lburst:
    add x20, x20, #1
    add x21, x21, #1
    str x21, [sp, #8]
    svc #29
.global cbs_wake_a_resume
cbs_wake_a_resume:
    ldp x9, x10, [sp]
    cmp x9, x19
    b.ne .Lunexpected
    cmp x10, x21
    b.ne .Lunexpected
    mrs x9, tpidr_el1
    cmp x9, x19
    b.ne .Lunexpected
    cmp x21, #5
    b.lo .Lburst
.Loverrun:
    add x20, x20, #1
    b .Loverrun
.size cbs_wake_a, . - cbs_wake_a

wake_entry cbs_wake_b, 0xb19
.Lneighbor:
    add x20, x20, #1
    b .Lneighbor
.size cbs_wake_b, . - cbs_wake_b

wake_entry cbs_wake_idle, 0xc19
.Lidle:
    add x20, x20, #1
    b .Lidle
.size cbs_wake_idle, . - cbs_wake_idle
