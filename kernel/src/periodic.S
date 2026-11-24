.global periodic_controller
periodic_controller:
    mov x19, #0xc19
    sub sp, sp, #16
    stp x19, xzr, [sp]
    mov x20, #1
    msr daifclr, #2
.global periodic_loop
periodic_loop:
    add x20, x20, #1
    b periodic_loop
.global periodic_loop_end
periodic_loop_end:

.macro worker entry, id
.global \entry
\entry:
    mov x0, #\id
    mov x1, sp
    bl {marker}
    svc #1
    b {fail}
.endm
worker periodic_a, 0
worker periodic_b, 1

.if {preempt}
.global periodic_running_a
periodic_running_a:
    mov x0, #2
    mov x1, sp
    bl {marker}
    mov x19, #0xa19
    sub sp, sp, #16
    stp x19, xzr, [sp]
    // Count the first unit before enabling IRQs. An already pending interrupt
    // can then sample a valid progress witness at the first ADD.
    mov x20, #1
    msr daifclr, #2
.global periodic_work
periodic_work:
    add x20, x20, #1
    ldr x9, [sp, #8]
    cbz x9, periodic_work
.global periodic_work_end
periodic_work_end:
    msr daifset, #2
    ldr x10, [sp]
    cmp x10, x19
    b.ne {fail}
    mov x0, #3
    add sp, sp, #16
    mov x1, sp
    mov x2, x20
    bl {marker}
    svc #1
    b {fail}
.endif
