#include <jnox/native_abi.h>

.section .text.entry,"ax",%progbits
.p2align 2
.global _start
.type _start,%function
_start:
    mov x29, xzr
    mov x30, xzr
    ldr x0, [sp]
    add x1, sp, #8
    bl main
.global __jnox_exit
.type __jnox_exit,%function
__jnox_exit:
    sxtw x0, w0
    mov x8, #JNOX_CALL_EXIT
    svc #JNOX_ABI_SVC
    // The kernel must never resume a terminated thread.
    brk #0
.size __jnox_exit, .-__jnox_exit
.size _start, .-_start

.section .note.jnox.abi,"a",%note
.p2align 2
.long 5, 4, 1
.asciz "JNOX"
.p2align 2
.long JNOX_ABI_VERSION
