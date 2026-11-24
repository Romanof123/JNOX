.section .bss.smp_handlers, "aw", %nobits
.balign 16
smp_handler_stacks:
    .skip 2 * {stack_size}

.section .rodata.smp_handlers, "a"
.balign 8
.global smp_handler_tops
smp_handler_tops:
    .quad smp_handler_stacks + {stack_size}
    .quad smp_handler_stacks + 2 * {stack_size}

.section .text.smp, "ax"
.balign 4
.global smp_task_entry
.type smp_task_entry, %function
smp_task_entry:
    // X19 identifies this task and epoch; X21 is its ID and X22 its epoch.
    sub sp, sp, #16
    stp x19, xzr, [sp]
    mov x0, x21
    mov x1, sp
    bl {enter}
    msr tpidr_el1, x19
    dup v31.2d, x19
    cbnz x21, .Lsmp_finish
    cmp x22, #1
    b.eq .Lsmp_global
    cmp x22, #2
    b.ne .Lsmp_finish
    svc #2
.global smp_cluster_resume
smp_cluster_resume:
    mov x0, sp
    bl {cluster_resume}
    b .Lsmp_finish
.Lsmp_global:
    adrp x12, {gates}
    add x12, x12, :lo12:{gates}
    lsl x20, x19, #32
    mov x1, #128
.Lsmp_work_before_timer:
    add x20, x20, #1
    subs x1, x1, #1
    b.ne .Lsmp_work_before_timer
    // CPU1 has already attempted to own this running task. The banked CPU0
    // one-shot will interrupt its non-yielding loop with every witness live.
    mrs x0, cntfrq_el0
    lsr x0, x0, #10
    msr cntp_tval_el0, x0
    mov x0, #1
    msr cntp_ctl_el0, x0
    isb
    msr daifclr, #2
.global smp_m_work_begin
smp_m_work_begin:
    add x20, x20, #1
    ldar x0, [x12]
    tbz x0, #{irq_bit}, smp_m_work_begin
.global smp_m_work_end
smp_m_work_end:
    msr daifset, #2
    mrs x0, mpidr_el1
    and x0, x0, #0xff
    cmp x0, #1
    b.ne .Lunexpected
    ldp x0, x1, [sp]
    cmp x0, x19
    b.ne .Lunexpected
    lsr x2, x20, #32
    cmp x2, x19
    b.ne .Lunexpected
    cbz x1, .Lunexpected
    sub x2, x20, x1
    cmp x2, #1
    b.hi .Lunexpected
    mrs x0, tpidr_el1
    cmp x0, x19
    b.ne .Lunexpected
    umov x0, v31.d[0]
    cmp x0, x19
    b.ne .Lunexpected
    umov x0, v31.d[1]
    cmp x0, x19
    b.ne .Lunexpected
    mov x0, sp
    mov x1, x19
    mov x2, x20
    bl {migrated}
.Lsmp_finish:
    svc #1
.global smp_after_finish
smp_after_finish:
    b .Lunexpected
.size smp_task_entry, . - smp_task_entry
