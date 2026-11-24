// X19 is a per-task cookie; X21 indexes its sleep command and wake timestamp.
.macro ee_worker name
.global ee_\name
.type ee_\name, %function
ee_\name:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    dup v31.2d, x19
    adrp x24, {command}
    add x24, x24, :lo12:{command}
    add x24, x24, x21, lsl #3
    adrp x25, {observation}
    add x25, x25, :lo12:{observation}
    add x25, x25, x21, lsl #3
    mov x20, #1
    msr daifclr, #2
.global ee_\name\()_work
ee_\name\()_work:
    add x20, x20, #1
    ldar x9, [x24]
    cbz x9, ee_\name\()_work
.global ee_\name\()_svc
ee_\name\()_svc:
    svc #1
.global ee_\name\()_wake
ee_\name\()_wake:
    // First instructions after returning to the blocked task's saved SVC PC.
    isb
    mrs x9, cntpct_el0
    stlr x9, [x25]
    b ee_\name\()_work
.size ee_\name, . - ee_\name
.endm

ee_worker a
ee_worker b
ee_worker c
