// Assembled only for JNOX_MUTEX_ANCHOR=selected-return.
// All general registers, SIMD, SP and system state have already been restored.
// x9..x14 still have byte-identical copies in the consumed original frame.
// No spill or modification of that frame is necessary.
.macro mutex_final_return
    mrs x13, nzcv
    adrp x9, jnox_mutex_anchor
    add x9, x9, :lo12:jnox_mutex_anchor
    ldr x10, [x9]
    cbz x10, .Lmutex_restore_scratch
    cmp x10, #1
    b.eq .Lmutex_pending
    // A completed token is inert on every later ordinal. Seeing it again
    // on or before the designated ordinal is an inconsistent transition.
    cmp x10, #3
    b.lo .Lmutex_identity_fail
    cmp x10, #4
    b.hi .Lmutex_identity_fail
    adrp x10, jnox_ap_returns
    ldr x10, [x10, :lo12:jnox_ap_returns]
    ldr x11, [x9, #16]
    cmp x10, x11
    b.ls .Lmutex_identity_fail
    b .Lmutex_restore_scratch
.Lmutex_pending:
    mrs x10, daif
    and x10, x10, #0x3c0
    cmp x10, #0x3c0
    b.ne .Lmutex_identity_fail
    adrp x10, jnox_mutex_generation
    ldr x10, [x10, :lo12:jnox_mutex_generation]
    ldr x11, [x9, #8]
    cbz x11, .Lmutex_identity_fail
    cmp x10, x11
    b.ne .Lmutex_identity_fail
    adrp x10, jnox_ap_returns
    ldr x10, [x10, :lo12:jnox_ap_returns]
    ldr x12, [x9, #16]
    cmp x10, x12
    b.ne .Lmutex_identity_fail
    add x11, x11, #1
    cmp x11, x12
    b.ne .Lmutex_identity_fail
    sub x10, sp, #{frame_size}
    ldr x11, [x9, #24]
    cmp x10, x11
    b.ne .Lmutex_identity_fail
    mrs x10, elr_el1
    ldr x11, [x9, #32]
    cmp x10, x11
    b.ne .Lmutex_identity_fail
    mrs x10, spsr_el1
    ldr x11, [x9, #40]
    cmp x10, x11
    b.ne .Lmutex_identity_fail
    mrs x10, tpidr_el1
    ldr x11, [x9, #48]
    cmp x10, x11
    b.ne .Lmutex_identity_fail
    cmp x10, #1
    b.ne .Lmutex_identity_fail
    ldr x11, [x9, #56]
    cbnz x11, .Lmutex_identity_fail
    cbnz x0, .Lmutex_identity_fail
    ldr x11, [x9, #72]
    cbz x11, .Lmutex_identity_fail
    mov x10, #2
    str x10, [x9]
    isb
    mrs x10, cntpct_el0
    adds x11, x10, x11
    b.cs .Lmutex_overflow
    msr cntp_cval_el0, x11
    mov x12, #1
    msr cntp_ctl_el0, x12
    isb
    mrs x12, cntp_cval_el0
    mrs x14, cntp_ctl_el0
    stp x10, x11, [x9, #80]
    stp x12, x14, [x9, #96]
    mov x10, #15
    str x10, [x9, #144]
    cmp x12, x11
    b.ne .Lmutex_readback_fail
    and x12, x14, #3
    cmp x12, #1
    b.ne .Lmutex_readback_fail
    mov x10, #3
    str x10, [x9]
    mov x10, #143
    str x10, [x9, #144]
    isb
    mrs x10, cntpct_el0
    str x10, [x9, #136]
.Lmutex_restore_scratch:
    msr nzcv, x13
    sub sp, sp, #{frame_size}
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldr x14, [sp, #112]
    ldr x9, [sp, #72]
    add sp, sp, #{frame_size}
    b .Lmutex_eret
.Lmutex_overflow:
    str x10, [x9, #80]
    mov x10, #3
    str x10, [x9, #144]
    mov x0, #21
    b .Lmutex_raw_stop
.Lmutex_readback_fail:
    mov x0, #20
    b .Lmutex_raw_stop
.Lmutex_identity_fail:
    mov x0, #19
.Lmutex_raw_stop:
    msr daifset, #15
    msr cntp_ctl_el0, xzr
    isb
    // Raw output may need stack space. Keep the selected task frame intact.
    adrp x9, __stack_top
    add x9, x9, :lo12:__stack_top
    mov sp, x9
    b jnox_mutex_arm_stop
.Lmutex_eret:
.endm
