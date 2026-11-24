.global periodic_controller
periodic_controller:
    mov x19, #0xc19
    sub sp, sp, #16
    stp x19, xzr, [sp]
    mov x20, #1
    msr daifclr, #2
.global periodic_loop
periodic_loop:
    add x20, x20, #1
    b periodic_loop
.global periodic_loop_end
periodic_loop_end:

.macro worker entry, id
.global \entry
\entry:
    mov x0, #\id
    mov x1, sp
    bl {marker}
    svc #1
    b {fail}
.endm
worker periodic_a, 0
worker periodic_b, 1
