.section .text.fixed_runtime, "ax"
.balign 4
.global fixed_runtime_enter
.type fixed_runtime_enter, %function
fixed_runtime_enter:
    blr x0
    svc #1
    brk #0
.size fixed_runtime_enter, . - fixed_runtime_enter

.global fixed_runtime_idle
.type fixed_runtime_idle, %function
fixed_runtime_idle:
    wfi
    b fixed_runtime_idle
.size fixed_runtime_idle, . - fixed_runtime_idle
