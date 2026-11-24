// The IRQ vector is the only path between these non-yielding task bodies.
// Each has distinct work PCs, progress in x20 and a private stack witness.
.macro cbs_task entry, begin, end, id, witness
.global \entry
.type \entry, %function
\entry:
    mov x0, #\id
    mov x1, #0
    mov x2, sp
    bl {marker}
    sub sp, sp, #16
    mov x19, #\witness
    lsl x20, x19, #32
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    add x20, x20, #1
    msr daifclr, #2
.global \begin
\begin:
    add x20, x20, #1
    ldr x9, [sp, #8]
    cbz x9, \begin
    // On first restore, an interrupted read may complete one iteration
    // with its stale zero. x20 can advance at most once before detection.
    msr daifset, #2
    sub x10, x20, x9
    cmp x10, #1
    b.hi .Lunexpected
    ldr x10, [sp]
    cmp x10, x19
    b.ne .Lunexpected
    mrs x10, tpidr_el1
    cmp x10, x19
    b.ne .Lunexpected
    cmp x19, #\witness
    b.ne .Lunexpected
    mov x0, #\id
    mov x1, #1
    mov x2, sp
    bl {marker}
    msr daifclr, #2
.Lkeep_working\@:
    add x20, x20, #1
    b .Lkeep_working\@
.global \end
\end:
.size \entry, . - \entry
.endm

cbs_task cbs_a, cbs_a_begin, cbs_a_end, 0, 0xa19
cbs_task cbs_b, cbs_b_begin, cbs_b_end, 1, 0xb19
cbs_task cbs_idle, cbs_idle_begin, cbs_idle_end, 2, 0xc19
