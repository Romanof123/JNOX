// The exception return provides x19, a private stack, and masked interrupts.
// Each runnable task does ordinary work until the physical timer interrupts it.
.macro vr_worker name
.global vr_\name
.type vr_\name, %function
vr_\name:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    dup v31.2d, x19
    mov x20, #1
    msr daifclr, #2
.global vr_\name\()_begin
vr_\name\()_begin:
    add x20, x20, #1
    b vr_\name\()_begin
.global vr_\name\()_end
vr_\name\()_end:
.size vr_\name, . - vr_\name
.endm

vr_worker a
vr_worker b
