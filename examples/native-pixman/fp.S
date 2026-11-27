/* Caller-saved SIMD registers plus FP controls across a real yield. */
#include "jnox/native_abi.h"
.text
.p2align 2
.global graphics_fp_yield
.type graphics_fp_yield,%function
graphics_fp_yield:
    mov x9, x0
    mvn x10, x9
    mrs x11, fpcr
    mrs x12, fpsr
    and x13, x9, #1
    add x13, x13, #1
    lsl x13, x13, #22
    and x14, x9, #1
    lsl x14, x14, #27
    msr fpcr, x13
    msr fpsr, x14
    mov v0.d[0], x9
    mov v0.d[1], x10
    mov v31.d[0], x10
    mov v31.d[1], x9
    mov x8, #JNOX_CALL_SCHED_YIELD
    svc #JNOX_ABI_SVC
    cbnz x0, 1f
    mov x1, v0.d[0]
    cmp x1, x9
    b.ne 1f
    mov x1, v0.d[1]
    cmp x1, x10
    b.ne 1f
    mov x1, v31.d[0]
    cmp x1, x10
    b.ne 1f
    mov x1, v31.d[1]
    cmp x1, x9
    b.ne 1f
    mrs x1, fpcr
    cmp x1, x13
    b.ne 1f
    mrs x1, fpsr
    cmp x1, x14
    b.eq 2f
1:  mov x0, #1
2:  msr fpcr, x11
    msr fpsr, x12
    ret
.section .note.GNU-stack,"",%progbits
