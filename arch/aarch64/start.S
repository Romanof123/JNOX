.section .text.boot, "ax"
.balign 4
.if {pi4_native}
.global pi4_native_payload_entry
.type pi4_native_payload_entry, %function
pi4_native_payload_entry:
.else
.global _start
.type _start, %function
_start:
.endif
    .if {cold}
    isb
    mrs x17, cntpct_el0
    .endif
    // The boot CPU owns the stack, BSS and UART. Both paths require an
    // identity-addressed firmware/loader handoff with MMU and D-cache off.
    msr daifset, #0xf
    .if {pi4}
    .if {pi4_smp}
    mrs x18, CurrentEL
    mrs x1, mpidr_el1
    ldr x2, =0xff00ffffff
    tst x1, x2
    b.ne .Lpark
    .endif
    // BCM2711 has a single cluster with Aff0 identifying each A72.
    mrs x0, mpidr_el1
    tst x0, #0xff
    b.ne .Lpark
    mrs x0, CurrentEL
    cmp x0, #8
    b.ne .Lcheck_el1
    // Bare-metal firmware handoff at EL2. No monitor or virtualization
    // state is preserved. Select AArch64 EL1, stage 2 off, FP accessible.
    mov x0, #(1 << 31)
    msr hcr_el2, x0
    .if {pi4_scheduler}
    // EL1PCTEN and EL1PCEN allow the NS EL1 physical counter and timer.
    // A direct EL1 loader must grant this access itself before entry.
    mrs x0, cnthctl_el2
    orr x0, x0, #3
    msr cnthctl_el2, x0
    .endif
    mov x0, #0x33ff
    msr cptr_el2, x0
    // Cortex-A72 SCTLR_EL1 RES1 bits, little-endian, MMU/caches off.
    ldr x0, =0x30d00800
    msr sctlr_el1, x0
    adr x0, .Lel1
    msr elr_el2, x0
    mov x0, #0x3c5
    msr spsr_el2, x0
    isb
    eret
.Lcheck_el1:
    .endif
    mrs x0, CurrentEL
    cmp x0, #4
    b.ne .Lpark
.Lel1:
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
    .if {cold}
    ldr x0, =jnox_cold_b
    str x17, [x0]
    .endif
    // Stack grows down from a 16-byte-aligned top. Rust never returns.
    .if {pi4_smp}
    ldr x0, =pi4_entry_el
    str x18, [x0]
    bl pi4_smp_translate
    cbz x0, .Lpark
    .endif
    b {rust_entry}
.Lpark:
    wfe
    b .Lpark
.if {pi4_native}
.size pi4_native_payload_entry, . - pi4_native_payload_entry
.else
.size _start, . - _start
.endif
