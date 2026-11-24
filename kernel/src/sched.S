.global sched_low_entry
.type sched_low_entry, %function
sched_low_entry:
    mov x0, sp
    bl {low_start}
    sub sp, sp, #16
    mov x15, #0x15a5
    mov x19, #0x19b6
    stp x15, x19, [sp]
    movi v31.16b, #0x55
    msr tpidr_el1, x19
    mov x0, #(1 << 22)
    msr fpcr, x0
    mov x0, #(1 << 27)
    msr fpsr, x0
    adrp x12, {done}
    add x12, x12, :lo12:{done}
    mov x9, xzr
    mov x11, xzr
    b sched_work_begin
.Larm:
    mrs x0, cntfrq_el0
    mov x1, #100
    udiv x0, x0, x1
    cbz x0, .Lunexpected
    msr cntp_tval_el0, x0
    mov x0, #1
    msr cntp_ctl_el0, x0
    isb
    mov x11, #1
    cmp xzr, xzr
    msr daifclr, #2
.global sched_work_begin
sched_work_begin:
    // Count actual work before arming. Once IRQs are enabled every possible
    // interrupted PC lies in these four non-yielding instructions.
    add x9, x9, #1
    cbz x11, .Larm
    ldar x10, [x12]
    cbz x10, sched_work_begin
.global sched_work_end
sched_work_end:
    // ADD, LDAR and CBZ have preserved the NZCV set before IRQ delivery.
    b.ne .Lunexpected
    b.cc .Lunexpected
    mov x0, #0x15a5
    cmp x15, x0
    b.ne .Lunexpected
    mov x0, #0x19b6
    cmp x19, x0
    b.ne .Lunexpected
    ldp x0, x1, [sp]
    cmp x0, x15
    b.ne .Lunexpected
    cmp x1, x19
    b.ne .Lunexpected
    mrs x0, tpidr_el1
    cmp x0, x19
    b.ne .Lunexpected
    mov x0, #0x5555555555555555
    umov x1, v31.d[0]
    umov x2, v31.d[1]
    cmp x0, x1
    b.ne .Lunexpected
    cmp x0, x2
    b.ne .Lunexpected
    mrs x0, fpcr
    cmp x0, #(1 << 22)
    b.ne .Lunexpected
    mrs x0, fpsr
    mov x1, #(1 << 27)
    cmp x0, x1
    b.ne .Lunexpected
    mov x0, sp
    b {pass}
.size sched_low_entry, . - sched_low_entry

// Each task has its own entry, stack, work PCs and post-SVC labels.
.macro high_task entry, resume, finish, id, witness, rr_begin, rr_end
.global \entry
.type \entry, %function
\entry:
    mov x0, sp
    mov x1, #\id
    mov x2, #0
    bl {high_start}
    sub sp, sp, #16
    stp xzr, xzr, [sp]
    .if {rr}
    // A's first slice resumes after two timer rotations. B's first and A's
    // second resume after three. Only the timer IRQ handler changes the count.
    mov x11, #3
    .if \id == {high_id}
    mov x11, #2
    .endif
.Lrr_setup\@:
    mov x19, #\witness
    stp x19, xzr, [sp]
    dup v31.2d, x19
    msr tpidr_el1, x19
    adrp x12, {rotations}
    add x12, x12, :lo12:{rotations}
    lsl x9, x19, #32
    mov x14, xzr
    b \rr_begin
.Lrr_enable\@:
    // Dispatch programmed the deadline with delivery masked. Unmask the
    // existing one-shot only after this task's witnesses and first count
    // are ready. A deadline that passed during restore or UART is due now.
    mrs x0, cntp_ctl_el0
    and x0, x0, #3
    cmp x0, #3
    b.ne .Lunexpected
    mov x0, #1
    msr cntp_ctl_el0, x0
    isb
    mov x14, #1
    msr daifclr, #2
.global \rr_begin
\rr_begin:
    add x9, x9, #1
    cbz x14, .Lrr_enable\@
    ldar x10, [x12]
    cmp x10, x11
    b.lo \rr_begin
.global \rr_end
\rr_end:
    msr daifset, #2
    // If the IRQ caught a stale rotation read, one final loop iteration can
    // run on return. Check the live X9 against the IRQ's exact saved value.
    ldp x0, x1, [sp]
    cbz x1, .Lunexpected
    sub x2, x9, x1
    cmp x2, #1
    b.hi .Lunexpected
    mov x1, #\witness
    cmp x19, x1
    b.ne .Lunexpected
    cmp x0, x1
    b.ne .Lunexpected
    mrs x0, tpidr_el1
    cmp x0, x1
    b.ne .Lunexpected
    umov x0, v31.d[0]
    cmp x0, x1
    b.ne .Lunexpected
    umov x0, v31.d[1]
    cmp x0, x1
    b.ne .Lunexpected
    .if \id == {high_id}
    cmp x11, #3
    b.eq .Lfinish_rr\@
    .endif
    mov x0, sp
    mov x1, #\id
    mov x2, #1
    bl {high_start}
    .if \id == {high_id}
    mov x11, #3
    b .Lrr_setup\@
    .endif
.Lfinish_rr\@:
    // All round-robin one-shots are now disabled by their IRQ handler.
    msr daifclr, #2
    .elseif {fifo}
    mov x19, #\witness
    str x19, [sp]
    svc #2
.global \resume
\resume:
    // The other peer installed a different X19 and used a different stack.
    mov x0, #\witness
    cmp x19, x0
    b.ne .Lunexpected
    ldr x1, [sp]
    cmp x1, x0
    b.ne .Lunexpected
    mov x0, sp
    mov x1, #\id
    mov x2, #1
    bl {high_start}
    .endif
    // Destroy the low witnesses in the live CPU registers before the SVC.
    mov x9, xzr
    mov x12, xzr
    mov x15, xzr
    mov x19, xzr
    movi v31.16b, #0
    msr fpcr, xzr
    msr fpsr, xzr
    msr tpidr_el1, xzr
    msr nzcv, xzr
    svc #1
.global \finish
\finish:
    b .Lunexpected
.size \entry, . - \entry
.endm

high_task sched_high_entry, sched_high_resume, sched_high_return, {high_id}, 0xa19, sched_rr_a_begin, sched_rr_a_end
.if {fifo}
high_task sched_b_entry, sched_b_resume, sched_b_return, {b_id}, 0xb19, sched_rr_b_begin, sched_rr_b_end
.endif
