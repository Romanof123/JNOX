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
    add x2, x1, x0, lsl #3
    add x2, x2, #8
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

.text
.p2align 2
.global __jnox_thread_start
.hidden __jnox_thread_start
.type __jnox_thread_start,%function
__jnox_thread_start:
    mov x9, x0
    mov x0, x1
    blr x9
    b pthread_exit
.size __jnox_thread_start, .-__jnox_thread_start

.global pthread_exit
.type pthread_exit,%function
pthread_exit:
    mov x8, #JNOX_CALL_THREAD_EXIT
    svc #JNOX_ABI_SVC
    brk #0
.size pthread_exit, .-pthread_exit

.section .note.jnox.abi,"a",%note
.p2align 2
.long 5, 4, 1
.asciz "JNOX"
.p2align 2
.long JNOX_ABI_VERSION
