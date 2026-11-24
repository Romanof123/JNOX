// Each thread has its own stack, cookie (X19), work counter (X20) and
// timer-expiry counter (address in X24). One CPU owns the dispatch clock.
.macro start_task name
.global \name
\name:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    adrp x24, {expiries}
    add x24, x24, :lo12:{expiries}
    add x24, x24, x23, lsl #3
    adrp x25, {one_tick}
    add x25, x25, :lo12:{one_tick}
    mov x20, #1
    msr daifclr, #2
.endm

// The loop ends only when the actual counter has passed the first complete
// service interval of this dispatch. The kernel independently measures it.
.macro one_interval
    ldar x10, [x25]
1:
    add x20, x20, #1
    isb
    mrs x11, cntpct_el0
    cmp x11, x10
    b.lo 1b
.endm

.macro until_expiries count
2:
    add x20, x20, #1
    ldar x10, [x24]
    cmp x10, #\count
    b.lo 2b
.endm

.macro timed_worker name, count
    start_task \name
.global \name\()_work
\name\()_work:
    until_expiries \count
.global \name\()_end
\name\()_end:
    svc #2
.global \name\()_sleep
\name\()_sleep:
    b .Lunexpected
.endm

timed_worker mlq_high_a, 2
timed_worker mlq_high_b, 2
timed_worker mlfq_cpu, 3

start_task mlq_low
    svc #2
.global mlq_low_sleep
mlq_low_sleep:
    b .Lunexpected

start_task mlfq_yielder
    one_interval
    svc #1
.global mlfq_after_yield_one
mlfq_after_yield_one:
    one_interval
    svc #1
.global mlfq_after_yield_two
mlfq_after_yield_two:
.global mlfq_yielder_work
mlfq_yielder_work:
    until_expiries 1
.global mlfq_yielder_end
mlfq_yielder_end:
    one_interval
    svc #2
.global mlfq_yielder_sleep
mlfq_yielder_sleep:
    b .Lunexpected

start_task mlfq_interactive
    one_interval
    svc #2
.global mlfq_interactive_wake
mlfq_interactive_wake:
    add x20, x20, #1
    svc #2
.global mlfq_interactive_end
mlfq_interactive_end:
    b .Lunexpected
