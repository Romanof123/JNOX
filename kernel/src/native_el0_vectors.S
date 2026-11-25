// Opt-in native AArch64 process entry. Rust validates the executable
// entry and complete user stack, installs the mappings, and quiesces IRQ sources.
// Boot enables FP/SIMD. EL1h supplies the kernel stack for every exception.
.section .text.native_el0,"ax",%progbits
.balign 2048
.global native_el0_vectors
.type native_el0_vectors,%object
.macro native_vector dest
    b \dest
    .space 124
.endm
native_el0_vectors:
    .rept 5                        // Current EL SP0 slots, then EL1h sync
        native_vector .Lunexpected
    .endr
    native_vector .Lidle_irq      // Current EL, SP_EL1 IRQ
    native_vector .Lunexpected
    native_vector .Lunexpected
    native_vector .Luser_sync     // Lower EL, AArch64 synchronous exception
    native_vector .Luser_irq
    .rept 6                        // Remaining A64 slots and all A32 slots
        native_vector .Lunexpected
    .endr
.size native_el0_vectors, .-native_el0_vectors

.global native_el0_enter
.type native_el0_enter,%function
native_el0_enter:
    msr daifset,#15
    mov x3,x2
    mrs x2,CurrentEL
    cmp x2,#4
    b.ne .Lunexpected
    mrs x2,SPSel
    cbz x2,.Lunexpected
    mov x2,sp
    tst x2,#15
    b.ne .Lunexpected
    tst x0,#3
    b.ne .Lunexpected
    tst x1,#15
    b.ne .Lunexpected
    adrp x2,native_el0_vectors
    add x2,x2,:lo12:native_el0_vectors
    msr vbar_el1,x2
    isb
    msr elr_el1,x0
    msr sp_el0,x1
    msr spsr_el1,x3
    msr tpidr_el0,xzr
    msr tpidrro_el0,xzr
    msr fpcr,xzr
    msr fpsr,xzr
    .irp reg,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
        movi v\reg\().16b,#0
    .endr
    .irp reg,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30
        mov x\reg,xzr
    .endr
    clrex
    isb
    eret
.size native_el0_enter,.-native_el0_enter

.macro native_integer_pairs op
    \op x0,x1,[sp,#0]
    \op x2,x3,[sp,#16]
    \op x4,x5,[sp,#32]
    \op x6,x7,[sp,#48]
    \op x8,x9,[sp,#64]
    \op x10,x11,[sp,#80]
    \op x12,x13,[sp,#96]
    \op x14,x15,[sp,#112]
    \op x16,x17,[sp,#128]
    \op x18,x19,[sp,#144]
    \op x20,x21,[sp,#160]
    \op x22,x23,[sp,#176]
    \op x24,x25,[sp,#192]
    \op x26,x27,[sp,#208]
    \op x28,x29,[sp,#224]
.endm
.macro native_simd_pairs op
    \op q0,q1,[sp,#304]
    \op q2,q3,[sp,#336]
    \op q4,q5,[sp,#368]
    \op q6,q7,[sp,#400]
    \op q8,q9,[sp,#432]
    \op q10,q11,[sp,#464]
    \op q12,q13,[sp,#496]
    \op q14,q15,[sp,#528]
    \op q16,q17,[sp,#560]
    \op q18,q19,[sp,#592]
    \op q20,q21,[sp,#624]
    \op q22,q23,[sp,#656]
    \op q24,q25,[sp,#688]
    \op q26,q27,[sp,#720]
    \op q28,q29,[sp,#752]
    \op q30,q31,[sp,#784]
.endm

.Lunexpected:
    msr daifset,#15
    bl native_el0_unexpected
.Lpark:
    wfe
    b .Lpark

.macro save_native_frame
    msr daifset,#15
    sub sp,sp,#832
    native_integer_pairs stp
    str x30,[sp,#240]
    mrs x9,esr_el1
    str x9,[sp,#248]
    mrs x9,spsr_el1
    mrs x10,elr_el1
    stp x9,x10,[sp,#256]
    mrs x9,sp_el0
    mrs x10,CurrentEL
    stp x9,x10,[sp,#272]
    mrs x9,fpcr
    mrs x10,fpsr
    stp x9,x10,[sp,#288]
    native_simd_pairs stp
    mrs x9,tpidr_el0
    mrs x10,tpidrro_el0
    str x9,[sp,#816]
    str x10,[sp,#824]
.endm

.Luser_irq:
    save_native_frame
    mov x0,sp
    bl native_sleep_irq
    b .Lrestore

.Luser_sync:
    save_native_frame
    mov x0,sp
    mov x1,#0x400
    bl native_el0_sync
.Lrestore:
    ldp x9,x10,[sp,#256]
    tst x9,#0x1f
    b.ne .Lunexpected
    msr spsr_el1,x9
    msr elr_el1,x10
    ldr x9,[sp,#272]
    msr sp_el0,x9
    ldp x9,x10,[sp,#288]
    msr fpcr,x9
    msr fpsr,x10
    ldr x9,[sp,#816]
    ldr x10,[sp,#824]
    msr tpidr_el0,x9
    msr tpidrro_el0,x10
    native_simd_pairs ldp
    native_integer_pairs ldp
    ldr x30,[sp,#240]
    add sp,sp,#832
    clrex
    eret

// Keep the calling Rust continuation and its NativeFrame on the process
// stack. x19 holds the process SP throughout idle (AAPCS callee-saved).
.global native_sleep_switch
.type native_sleep_switch,%function
native_sleep_switch:
    stp x19,x30,[sp,#-16]!
    mov x19,sp
    mov sp,x0
    msr daifclr,#2
    isb
.global native_sleep_idle
native_sleep_idle:
    wfi
    b native_sleep_idle
.size native_sleep_switch,.-native_sleep_switch

.Lidle_irq:
    // IRQ delivery at EL1h is enabled only inside the assembly idle loop.
    // Its only live value is x19. IRQ entry masked delivery before this call.
    mov x0,xzr
    bl native_sleep_irq
    cbz x0,.Lidle_eret
    adr x9,.Lleave_idle
    msr elr_el1,x9
    mrs x9,spsr_el1
    orr x9,x9,#0x80
    msr spsr_el1,x9
.Lidle_eret:
    eret

.Lleave_idle:
    mov sp,x19
.global native_sleep_resume
native_sleep_resume:
    ldp x19,x30,[sp],#16
    ret
