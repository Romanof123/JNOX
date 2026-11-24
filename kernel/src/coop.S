// Task-private stack slots hold the identity and (for A) the interrupted count.
.macro task_stack witness
    sub sp, sp, #16
    mov x19, #\witness
    stp x19, xzr, [sp]
.endm

.macro check_task witness
    mov x0, #\witness
    cmp x19, x0
    b.ne .Lunexpected
    ldr x1, [sp]
    cmp x1, x0
    b.ne .Lunexpected
.endm

.macro report marker_id
    mov x0, #\marker_id
    mov x1, sp
    bl {marker}
.endm

.global coop_a_entry
.type coop_a_entry, %function
coop_a_entry:
    task_stack {a_witness}
    report 0
    adrp x12, {released}
    add x12, x12, :lo12:{released}
    mov x20, xzr
    mov x14, xzr
    b coop_a_work_begin
.Larm_release:
    // A has performed one iteration. A one-tick CNTP deadline releases B.
    // Keep delivery masked until the first instruction of the work range.
    mov x0, #1
    msr cntp_tval_el0, x0
    msr cntp_ctl_el0, x0
    isb
    mov x14, #1
    msr daifclr, #2
.global coop_a_work_begin
coop_a_work_begin:
    add x20, x20, #1
    cbz x14, .Larm_release
    ldar x0, [x12]
    cbz x0, coop_a_work_begin
.global coop_a_work_end
coop_a_work_end:
    // A private stack snapshot records X20 at the IRQ. Depending on the
    // interrupted instruction, A can complete at most one further increment.
    ldr x0, [sp, #8]
    cbz x0, .Lunexpected
    sub x1, x20, x0
    cmp x1, #1
    b.hi .Lunexpected
    check_task {a_witness}
    report 1
    svc #2                          // Yield to the older ready task.
.global coop_a_after_yield
coop_a_after_yield:
    check_task {a_witness}
    report 3
    svc #4                          // Wake B, retain A.
.global coop_a_after_wake
coop_a_after_wake:
    check_task {a_witness}
    report 4
    svc #1
.global coop_a_after_exit
coop_a_after_exit:
    b .Lunexpected
.size coop_a_entry, . - coop_a_entry

.global coop_b_entry
.type coop_b_entry, %function
coop_b_entry:
    task_stack {b_witness}
    report 2
    svc #3                          // Block until A explicitly wakes B.
.global coop_b_after_block
coop_b_after_block:
    check_task {b_witness}
    report 5
    svc #1
.global coop_b_after_exit
coop_b_after_exit:
    b .Lunexpected
.size coop_b_entry, . - coop_b_entry

.global coop_event_entry
.type coop_event_entry, %function
coop_event_entry:
    mov x0, sp
    b {events}
.size coop_event_entry, . - coop_event_entry
