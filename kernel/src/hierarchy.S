.macro leaf id
.global h_leaf\id
h_leaf\id:
    sub sp, sp, #16
    mov x19, #(0x610 + \id)
    stp x19, xzr, [sp]
    mov x20, #1
    msr tpidr_el1, x19
    svc #2
.global h_leaf\id\()_wake
h_leaf\id\()_wake:
    ldr x9, [sp]
    cmp x19, x9
    b.ne .Lunexpected
    msr daifclr, #2
.global h_leaf\id\()_work
h_leaf\id\()_work:
    add x20, x20, #1
    b h_leaf\id\()_work
.global h_leaf\id\()_end
h_leaf\id\()_end:
.endm
leaf 0
leaf 1
leaf 2
leaf 3

.global h_idle
h_idle:
    sub sp, sp, #16
    mov x19, #0x614
    stp x19, xzr, [sp]
    mov x20, #1
    msr tpidr_el1, x19
    msr daifclr, #2
.global h_idle_work
h_idle_work:
    add x20, x20, #1
    b h_idle_work
.global h_idle_end
h_idle_end:
