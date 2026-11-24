.global overrun_idle
overrun_idle:
    mov x20, #0
    msr daifclr, #2
.global overrun_idle_loop
overrun_idle_loop:
    add x20, x20, #1
    b overrun_idle_loop
.global overrun_short
overrun_short:
    mov x20, #0
1:  add x20, x20, #1
    cmp x20, #128
    b.lo 1b
    svc #1
.global overrun_short_return
overrun_short_return:
    b {fail}

.global overrun_long
overrun_long:
    mov x20, #1
    mov x21, #0
    msr daifclr, #2
.global overrun_work
overrun_work:
    add x20, x20, #1
    cbz x21, overrun_work
.global overrun_work_end
overrun_work_end:
    msr daifset, #2
    svc #1
.global overrun_long_return
overrun_long_return:
    b {fail}
