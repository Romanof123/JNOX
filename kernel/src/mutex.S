// X19 and the first private stack word identify the context. Each SVC saves
// a real continuation. Blocked acquisitions return only on handoff or expiry.
.macro thread name
.global \name
.type \name, %function
\name:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
.endm

.macro lock id, continuation, status=0
    mov x0, #\id
    svc #1
.global \continuation
\continuation:
    cmp x0, #\status
    b.ne .Lunexpected
.endm

.macro unlock id
    mov x0, #\id
    svc #2
.endm

.macro release_tasks mask
    mov x0, #\mask
    svc #3
.endm

.macro finish
    svc #5
    b .Lunexpected
.endm

thread mutex_classic_low
    lock 0, mutex_classic_low_held
    release_tasks 12                // MEDIUM, then HIGH
    unlock 0
    finish

thread mutex_classic_high
    lock 0, mutex_classic_handoff
    unlock 0
    finish

thread mutex_medium
    finish

thread mutex_chain_base
    lock 1, mutex_base_held
    release_tasks 2                 // LOW subsequently waits on BASE
    release_tasks 12                // the wait chain already exists
    adrp x21, {expired}
    add x21, x21, :lo12:{expired}
    mov x20, #1
    msr daifclr, #2
.global mutex_expiry_begin
mutex_expiry_begin:
    add x20, x20, #1
    ldar x0, [x21]
    cbz x0, mutex_expiry_begin
.global mutex_expiry_end
mutex_expiry_end:
    msr daifset, #2
    ldr x1, [sp, #8]                // value sampled by the hardware IRQ
    cbz x1, .Lunexpected
    sub x1, x20, x1
    cmp x1, #1
    b.hi .Lunexpected
    unlock 1
    finish

thread mutex_chain_low
    lock 0, mutex_chain_low_held
    lock 1, mutex_chain_handoff
    unlock 1
    unlock 0
    finish

thread mutex_chain_high
    lock 0, mutex_high_timeout, 1
    finish

thread mutex_ceiling_low
    lock 0, mutex_outer_held
    release_tasks 4                 // MEDIUM is runnable
    lock 1, mutex_inner_held
    release_tasks 8                 // HIGH ties the ceiling and waits its turn
    svc #4                          // explicitly yield to the equal-priority peer
    unlock 1
    unlock 0
    finish

thread mutex_ceiling_high
    lock 1, mutex_ceiling_handoff
    unlock 1
    finish
