// All five private stacks use the same saved-frame path. X19, Q31 and TLS
// carry the context's identity. X20 counts instructions in its work loop.
.macro ext_enter
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    dup v31.2d, x19
    mov x20, xzr
.endm

.macro service name, idle
.global ext_\name
.type ext_\name, %function
ext_\name:
    ext_enter
    mov x21, xzr
    adrp x24, {epoch}
    add x24, x24, :lo12:{epoch}
    adrp x25, {observed}
    add x25, x25, :lo12:{observed}
    msr daifclr, #2
.global ext_\name\()_work
ext_\name\()_work:
    add x20, x20, #1
    ldar x10, [x24]
    cmp x10, x21
    b.eq 1f
    sub x12, x10, #1
    add x12, x25, x12, lsl #3
    isb
    mrs x11, cntpct_el0
    stlr x11, [x12]
    mov x21, x10
    .if {normal}
    mov x0, x19
    mov x1, x21
    bl normal_job_tick
    dup v31.2d, x19
    .endif
1:
    .if \idle
    wfi
    .endif
    b ext_\name\()_work
.global ext_\name\()_end
ext_\name\()_end:
.size ext_\name, . - ext_\name
.endm

service reserved, 0
service fallback, 0
service nominee, 0
service idle, 1

.global ext_eval_live
.type ext_eval_live, %function
ext_eval_live:
    ext_enter
    msr daifclr, #2
    bl {evaluate}
    // The bounded evaluator masks IRQs to publish the completed transaction.
    // Restore the caller-saved SIMD witness before exposing its terminal loop.
    dup v31.2d, x19
    msr daifclr, #2
.global ext_eval_live_work
ext_eval_live_work:
    add x20, x20, #1
    b ext_eval_live_work
.global ext_eval_live_end
ext_eval_live_end:
.size ext_eval_live, . - ext_eval_live

.global ext_eval_stopped
.type ext_eval_stopped, %function
ext_eval_stopped:
    ext_enter
    msr daifclr, #2
.global ext_eval_stopped_work
ext_eval_stopped_work:
    // A stopped producer never calls the evaluator or publishes a heartbeat.
    b ext_eval_stopped_work
.global ext_eval_stopped_end
ext_eval_stopped_end:
.size ext_eval_stopped, . - ext_eval_stopped
