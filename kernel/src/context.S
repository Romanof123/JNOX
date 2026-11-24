.section .text.sched, "ax"
.balign 2048
.global sched_vectors
.type sched_vectors, %object
.macro vector target
    b \target
    .space 124
.endm
sched_vectors:
    // Current EL with SP_EL0
    vector .Lunexpected
    vector .Lunexpected
    vector .Lunexpected
    vector .Lunexpected
    // Current EL with SP_EL1
    vector .Lsync
    vector .Lirq
    vector .Lunexpected
    vector .Lunexpected
    // Lower EL, AArch64 / AArch32
    .rept 8
    vector .Lunexpected
    .endr
.size sched_vectors, . - sched_vectors

.Lunexpected:
    b {fail}

.macro integer_pairs op
    \op x0,  x1,  [sp, #0]
    \op x2,  x3,  [sp, #16]
    \op x4,  x5,  [sp, #32]
    \op x6,  x7,  [sp, #48]
    \op x8,  x9,  [sp, #64]
    \op x10, x11, [sp, #80]
    \op x12, x13, [sp, #96]
    \op x14, x15, [sp, #112]
    \op x16, x17, [sp, #128]
    \op x18, x19, [sp, #144]
    \op x20, x21, [sp, #160]
    \op x22, x23, [sp, #176]
    \op x24, x25, [sp, #192]
    \op x26, x27, [sp, #208]
    \op x28, x29, [sp, #224]
.endm

// The same offset list drives save and restore. AAPCS caller- and
// callee-saved SIMD registers are equally live at an asynchronous IRQ.
.macro simd_pairs op
    \op q0,  q1,  [sp, #({simd} + 0)]
    \op q2,  q3,  [sp, #({simd} + 32)]
    \op q4,  q5,  [sp, #({simd} + 64)]
    \op q6,  q7,  [sp, #({simd} + 96)]
    \op q8,  q9,  [sp, #({simd} + 128)]
    \op q10, q11, [sp, #({simd} + 160)]
    \op q12, q13, [sp, #({simd} + 192)]
    \op q14, q15, [sp, #({simd} + 224)]
    \op q16, q17, [sp, #({simd} + 256)]
    \op q18, q19, [sp, #({simd} + 288)]
    \op q20, q21, [sp, #({simd} + 320)]
    \op q22, q23, [sp, #({simd} + 352)]
    \op q24, q25, [sp, #({simd} + 384)]
    \op q26, q27, [sp, #({simd} + 416)]
    \op q28, q29, [sp, #({simd} + 448)]
    \op q30, q31, [sp, #({simd} + 480)]
.endm

.macro handle callback
    sub sp, sp, #{frame_size}
    integer_pairs stp
    str x30, [sp, #240]
    simd_pairs stp
    mrs x9, elr_el1
    mrs x10, spsr_el1
    stp x9, x10, [sp, #{elr}]
    mrs x9, fpcr
    mrs x10, fpsr
    stp x9, x10, [sp, #{fpcr}]
    mrs x9, tpidr_el1
    mrs x10, sp_el0
    stp x9, x10, [sp, #{tpidr_el1}]
    mrs x9, tpidr_el0
    mrs x10, tpidrro_el0
    stp x9, x10, [sp, #{tpidr_el0}]
    str xzr, [sp, #({simd} - 8)]
    mov x0, sp
    bl \callback
    b sched_start_frame
.endm

.Lirq:
    handle {irq}
.Lsync:
    handle {svc}

.global sched_start_frame
.type sched_start_frame, %function
sched_start_frame:
    // The Rust callback has returned: its temporary borrows and stack
    // frame are gone. Change SP under the hardware exception masks.
    mov sp, x0
    ldp x9, x10, [sp, #{elr}]
    msr elr_el1, x9
    msr spsr_el1, x10
    ldp x9, x10, [sp, #{fpcr}]
    msr fpcr, x9
    msr fpsr, x10
    ldp x9, x10, [sp, #{tpidr_el1}]
    msr tpidr_el1, x9
    msr sp_el0, x10
    ldp x9, x10, [sp, #{tpidr_el0}]
    msr tpidr_el0, x9
    msr tpidrro_el0, x10
    simd_pairs ldp
    integer_pairs ldp
    ldr x30, [sp, #240]
    add sp, sp, #{frame_size}
    clrex
    eret
.size sched_start_frame, . - sched_start_frame
