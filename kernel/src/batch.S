// Initial X19 is distinct for every epoch/task pair and each context has its
// own 16-KiB stack. The second reserved word records A's interrupted count.
.macro batch_enter id
    sub sp, sp, #16
    stp x19, xzr, [sp]
    mov x0, #\id
    mov x1, #0
    mov x2, sp
    bl {marker}
    dup v31.2d, x19
    msr tpidr_el1, x19
.endm

.global batch_a_entry
.type batch_a_entry, %function
batch_a_entry:
    batch_enter {a}
    adrp x12, {released}
    add x12, x12, :lo12:{released}
    mov x20, xzr
    mov x14, xzr
    b batch_a_work_begin
.Lbatch_arm:
    // Count work before arming. A one-counter-tick deadline triggers the
    // physical interrupt independently of the workload's logical time.
    mov x0, #1
    msr cntp_tval_el0, x0
    msr cntp_ctl_el0, x0
    isb
    mov x14, #1
    msr daifclr, #2
.global batch_a_work_begin
batch_a_work_begin:
    add x20, x20, #1
    cbz x14, .Lbatch_arm
    ldar x0, [x12]
    cbz x0, batch_a_work_begin
.global batch_a_work_end
batch_a_work_end:
    ldp x0, x1, [sp]
    cmp x19, x0
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
    mov x0, #{a}
    mov x1, #1
    mov x2, sp
    bl {marker}
    svc #1
.global batch_a_after_exit
batch_a_after_exit:
    b .Lunexpected
.size batch_a_entry, . - batch_a_entry

.macro batch_peer entry, id, finish
.global \entry
.type \entry, %function
\entry:
    batch_enter \id
    svc #1
.global \finish
\finish:
    b .Lunexpected
.size \entry, . - \entry
.endm

batch_peer batch_b_entry, {b}, batch_b_after_exit
batch_peer batch_c_entry, {c}, batch_c_after_exit
