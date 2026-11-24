.section .text.power, "ax"
.balign 4
.global power_work
.type power_work, %function
// x0 points to six u64s owned by the calling CPU. Sum the same input in
// every foreground and background invocation, with the stack used in-loop.
power_work:
    mov x4, x1
    sub sp, sp, #16
    str xzr, [sp]
    mrs x1, mpidr_el1
    mov x2, sp
    stp x1, x2, [x0]
    isb
    mrs x1, cntpct_el0
    str x1, [x0, #16]
    mov x2, #4096
.global power_work_loop
power_work_loop:
    ldr x3, [sp]
    add x3, x3, x2
    str x3, [sp]
    // On the shared case only, the peer performs a full background batch
    // between the two halves. Its time is included in observed response.
    cbz x4, 2f
    cmp x2, #2048
    b.ne 2f
    mov x5, #2
    stlr x5, [x4]
1:
    yield
    ldar x5, [x4]
    cmp x5, #3
    b.ne 1b
2:
    subs x2, x2, #1
    b.ne power_work_loop
    isb
    mrs x1, cntpct_el0
    str x1, [x0, #24]
    str x3, [x0, #32]
    mov x2, #4096
    str x2, [x0, #40]
    add sp, sp, #16
    ret
.global power_work_end
power_work_end:
.size power_work, . - power_work
