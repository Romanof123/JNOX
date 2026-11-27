/* Single-threaded graphics-runtime entry. The loader supplies argc/argv and
 * zeroed private BSS. ABI note and entry address match the ordinary SDK. */
#include "jnox/native_abi.h"
.section .text.entry,"ax",%progbits
.p2align 2
.global _start
_start:
    mov x29, xzr
    mov x30, xzr
    ldr x0, [sp]
    add x1, sp, #8
    bl jnox_graphics_start
    brk #0
.section .note.jnox.abi,"a",%note
.p2align 2
.long 5, 4, 1
.asciz "JNOX"
.p2align 2
.long JNOX_ABI_VERSION
.section .note.GNU-stack,"",%progbits
