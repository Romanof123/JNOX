.section .text.boot, "ax"
.balign 4
.global _start
.type _start, %function
_start:
    // Direct ELF boot on virt, one Cortex-A72, EL2/EL3 off, MMU off.
    // No other core or interrupt handler may use the stack, BSS or UART.
    msr daifset, #0xf
    mrs x0, CurrentEL
    cmp x0, #4
    b.ne .Lpark
    msr spsel, #1
    ldr x0, =__stack_top
    mov sp, x0

    // aarch64-unknown-none permits FP/SIMD in compiler-generated code.
    mrs x0, cpacr_el1
    orr x0, x0, #(3 << 20)
    msr cpacr_el1, x0
    isb

    // QEMU may already have zeroed BSS. Dirty this aligned u64 so Rust's
    // subsequent volatile read observes whether our clear reached it.
    ldr x0, ={bss_probe}
    mov x1, #-1
    str x1, [x0]

    // link.ld aligns both bounds to 8 and puts the stack outside the range.
    ldr x0, =__bss_start
    ldr x1, =__bss_end
.Lclear:
    cmp x0, x1
    b.hs .Lenter
    str xzr, [x0], #8
    b .Lclear
.Lenter:
    // Stack grows down from a 16-byte-aligned top. Rust never returns.
    b {rust_entry}
.Lpark:
    wfe
    b .Lpark
.size _start, . - _start
