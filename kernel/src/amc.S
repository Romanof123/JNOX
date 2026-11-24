// Each entry owns its initial frame, private stack, TLS and SIMD witness.
// A finite job leaves its computed result in x23 until the slot timer fires.
.macro amc_enter
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    dup v31.2d, x19
    mov x20, #1
    mov x23, #0
.endm

.macro finite_job name, terms
.global amc_\name
.type amc_\name, %function
amc_\name:
    amc_enter
    mov x9, #\terms
1:
    add x23, x23, x9
    subs x9, x9, #1
    b.ne 1b
    msr daifclr, #2
.global amc_\name\()_begin
amc_\name\()_begin:
    add x20, x20, #1
    b amc_\name\()_begin
.global amc_\name\()_end
amc_\name\()_end:
.size amc_\name, . - amc_\name
.endm

finite_job lo_initial, 8
finite_job lo_pending, 8
finite_job survivor, 32
finite_job lo_fresh, 16

// The overrun job never yields, finishes, or calls the kernel. After the
// transition IRQ returns to its interrupted instruction, it samples the
// physical counter on first observing HI mode. No UART runs in this window.
.global amc_hi
.type amc_hi, %function
amc_hi:
    amc_enter
    mov x24, #0
    adrp x10, {signal}
    add x10, x10, :lo12:{signal}
    adrp x11, {observation}
    add x11, x11, :lo12:{observation}
    msr daifclr, #2
.global amc_hi_begin
amc_hi_begin:
    add x20, x20, #1
    cbnz x24, amc_hi_begin
    ldar x9, [x10]
    cbz x9, amc_hi_begin
    isb
    mrs x9, cntpct_el0
    stlr x9, [x11]
    mov x24, #1
    b amc_hi_begin
.global amc_hi_end
amc_hi_end:
.size amc_hi, . - amc_hi
