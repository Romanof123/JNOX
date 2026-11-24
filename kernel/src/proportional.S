// Each task's private word, TPIDR and X19 agree across all preemptions.
.macro start_worker name
.global \name
\name:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    mov x20, #1
.endm

.macro cpu_worker name
    start_worker \name
    msr daifclr, #2
.global \name\()_work
\name\()_work:
    add x20, x20, #1
    b \name\()_work
.global \name\()_end
\name\()_end:
.endm

cpu_worker prop_cpu_a
cpu_worker prop_cpu_b
cpu_worker prop_cpu_c

start_worker prop_yielder
    adrp x24, {counter_target}
    add x24, x24, :lo12:{counter_target}
    msr daifclr, #2
.Lyielder_start:
    ldar x10, [x24]
.Lyielder_work:
    add x20, x20, #1
    isb
    mrs x11, cntpct_el0
    cmp x11, x10
    b.lo .Lyielder_work
    svc #1
.global prop_yield_return
prop_yield_return:
    b .Lyielder_start

// Initial SPSR masks IRQ. CNTP still asserts after the ordinary two-tick
// grant; keep doing work on this stack until a third full interval elapses.
start_worker prop_delayed
    adrp x24, {counter_target}
    add x24, x24, :lo12:{counter_target}
    ldar x10, [x24]
.Ldelay_masked:
    add x20, x20, #1
    isb
    mrs x11, cntpct_el0
    cmp x11, x10
    b.lo .Ldelay_masked
    msr daifclr, #2
.global prop_delayed_work
prop_delayed_work:
    add x20, x20, #1
    b prop_delayed_work
.global prop_delayed_end
prop_delayed_end:
