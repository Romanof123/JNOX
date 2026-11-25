.section .text.pi4_native_shim,"ax",%progbits
.balign 4
.global _start
.type _start,%function
_start:
    msr daifset, #0xf
    mrs x0, mpidr_el1
    ldr x1, =0xff00ffffff
    tst x0, x1
    b.ne .Lpi4_native_shim_park

    // The existing Pi cold entry requires identity addresses and MMU/D-cache
    // disabled. Reject a different handoff before accessing image memory.
    mrs x0, CurrentEL
    cmp x0, #8
    b.eq .Lpi4_native_shim_el2
    cmp x0, #4
    b.ne .Lpi4_native_shim_park
    mrs x0, sctlr_el1
    b .Lpi4_native_shim_check
.Lpi4_native_shim_el2:
    mrs x0, sctlr_el2
.Lpi4_native_shim_check:
    tst x0, #1
    b.ne .Lpi4_native_shim_park
    tst x0, #4
    b.ne .Lpi4_native_shim_park

    ldr x0, =__native_payload_lma
    ldr x1, =__native_payload_start
    ldr x2, =__native_payload_size
    lsr x3, x2, #3
    cbz x3, .Lpi4_native_shim_tail
.Lpi4_native_shim_words:
    ldr x4, [x0], #8
    str x4, [x1], #8
    subs x3, x3, #1
    b.ne .Lpi4_native_shim_words
.Lpi4_native_shim_tail:
    and x2, x2, #7
    cbz x2, .Lpi4_native_shim_sync
.Lpi4_native_shim_bytes:
    ldrb w4, [x0], #1
    strb w4, [x1], #1
    subs x2, x2, #1
    b.ne .Lpi4_native_shim_bytes
.Lpi4_native_shim_sync:
    // x1 is the exclusive destination end. CTR_EL0.DminLine gives the
    // smallest data/unified cache line in words; start is page aligned.
    dsb sy
    mrs x2, ctr_el0
    ubfx x2, x2, #16, #4
    mov x3, #4
    lsl x3, x3, x2
    ldr x0, =__native_payload_start
    mov x4, x0
.Lpi4_native_shim_clean:
    dc cvau, x4
    add x4, x4, x3
    cmp x4, x1
    b.lo .Lpi4_native_shim_clean
    dsb sy
    ic iallu
    dsb sy
    isb
    br x0
.Lpi4_native_shim_park:
    wfe
    b .Lpi4_native_shim_park
.size _start, . - _start
.ltorg
