// Typed AAPCS64 entry. x0 is the cell copied by Transaction::begin while masked.
// 0 ordinary, 1 dormant, 2 A active, 3 B active. Selectors are link inputs.
#ifndef JNOX_EXTENSION_STOP_VARIANT
#define JNOX_EXTENSION_STOP_VARIANT 0
#endif
#if JNOX_EXTENSION_STOP_VARIANT < 0 || JNOX_EXTENSION_STOP_VARIANT > 3
#error Invalid native extension evaluator entry variant
#endif

.section .jnox_extension_entry,"ax",%progbits
.balign 64
.global jnox_extension_evaluator_enter
.type jnox_extension_evaluator_enter,%function
jnox_extension_evaluator_enter:
    msr daifclr,#2
    isb
#if JNOX_EXTENSION_STOP_VARIANT
    adr x16,jnox_native_extension_stop_cells
    ldp x16,x17,[x16]
    eor x16,x16,x0
    cbz x16,.Lstopped
    eor x17,x17,x0
    cbz x17,.Lstopped
#endif
    ret
#if JNOX_EXTENSION_STOP_VARIANT
.global jnox_native_extension_stop_loop
.type jnox_native_extension_stop_loop,%function
jnox_native_extension_stop_loop:
.Lstopped:
    b .Lstopped
.size jnox_native_extension_stop_loop,.-jnox_native_extension_stop_loop
#endif
// .org fails assembly if the body ever exceeds the reserved extent.
.org jnox_extension_evaluator_enter + 64, 0
.size jnox_extension_evaluator_enter,.-jnox_extension_evaluator_enter

#if JNOX_EXTENSION_STOP_VARIANT
.section .jnox_extension_selector,"a",%progbits
.balign 8
.global jnox_native_extension_stop_cells
.type jnox_native_extension_stop_cells,%object
jnox_native_extension_stop_cells:
#if JNOX_EXTENSION_STOP_VARIANT == 1
    .quad -1,-1
#elif JNOX_EXTENSION_STOP_VARIANT == 2
    .quad 1,3
#else
    .quad 9,-1
#endif
.size jnox_native_extension_stop_cells,.-jnox_native_extension_stop_cells
#endif
