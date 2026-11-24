// Each context gets a distinct stack and cookie in X19. X20 counts
// instructions completed by the running worker and survives real exceptions.
.macro enter
    sub sp, sp, #16
    stp x19, xzr, [sp]
    dup v31.2d, x19
    msr tpidr_el1, x19
    mov x20, #1
.endm

.global priority_resume_a_entry
priority_resume_a_entry:
    enter
    adrp x12, {gate}
    add x12, x12, :lo12:{gate}
    msr daifclr, #2
.global priority_resume_a_work
priority_resume_a_work:
    add x20, x20, #1
    ldar x9, [x12]
    cbz x9, priority_resume_a_work
    cbnz x21, priority_resume_a_finish
    svc #2
.global priority_resume_after_yield
priority_resume_after_yield:
    add x20, x20, #1
priority_resume_a_finish:
    svc #1
.global priority_resume_a_after_exit
priority_resume_a_after_exit:
    b .Lunexpected

.macro peer entry, finish
.global \entry
\entry:
    enter
    msr daifclr, #2
    mov x10, #4096
1:
    add x20, x20, #1
    sub x10, x10, #1
    cbnz x10, 1b
    svc #1
.global \finish
\finish:
    b .Lunexpected
.endm
peer priority_resume_b_entry, priority_resume_b_after_exit
peer priority_resume_high_entry, priority_resume_high_after_exit
