// The IRQ vector is the only path between these non-yielding task bodies.
// Each has distinct work PCs, progress in x20 and a private stack witness.
.macro part_check branch, code, actual, expected
    .if {diagnostic}
        \branch .Lpart_bad\@
        b .Lpart_good\@
.Lpart_bad\@:
        mov x0, #\code
        mov x1, \actual
        mov x2, \expected
        mov x3, #-1
        b partition_diag_stop
.Lpart_good\@:
    .else
        \branch .Lunexpected
    .endif
.endm
.macro part_task entry, begin, end, id, witness
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
    part_check b.hi, 41, x10, #1
    ldr x10, [sp]
    cmp x10, x19
    part_check b.ne, 42, x10, x19
    mrs x10, tpidr_el1
    cmp x10, x19
    part_check b.ne, 43, x10, x19
    cmp x19, #\witness
    part_check b.ne, 44, x19, #\witness
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

part_task part_a, part_a_begin, part_a_end, 0, 0xa19
part_task part_b, part_b_begin, part_b_end, 1, 0xb19
part_task part_idle, part_idle_begin, part_idle_end, 2, 0xc19
