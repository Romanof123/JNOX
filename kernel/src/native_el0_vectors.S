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
.ifdef NATIVE_AMC
.set NATIVE_EARLY_ENTRY,1
.endif
.ifdef NATIVE_WEIGHTED_CONTINUOUS
.set NATIVE_EARLY_ENTRY,1
.endif
.ifdef NATIVE_BATCH
.set NATIVE_EARLY_ENTRY,1
.endif
.ifdef NATIVE_EARLY_ENTRY
// Keep the earliest ordered exception timestamp before the common frame save.
// The normal 832-byte frame remains at SP for the Rust handlers. The extra
// 16 bytes above it are private to this vector and removed before EL0 return.
.macro weighted_user_vector dest
    str x2,[sp,#-16]!
    isb
    mrs x2,cntpct_el0
    str x2,[sp,#8]
    ldr x2,[sp]
    b \dest
    .space 128-24
.endm
.endif
native_el0_vectors:
    .rept 5                        // Current EL SP0 slots, then EL1h sync
        native_vector .Lunexpected
    .endr
    .ifdef NATIVE_EARLY_ENTRY
    isb
    mrs x1,cntpct_el0
    b .Lidle_irq
    .space 128-12
    .else
    native_vector .Lidle_irq
    .endif
    // Current EL, SP_EL1 IRQ
    native_vector .Lunexpected
    native_vector .Lunexpected
    .ifdef NATIVE_EARLY_ENTRY
    weighted_user_vector .Luser_sync
    weighted_user_vector .Luser_irq
    .else
    native_vector .Luser_sync     // Lower EL, AArch64 synchronous exception
    native_vector .Luser_irq
    .endif
    .rept 6                        // Remaining A64 slots and all A32 slots
        native_vector .Lunexpected
    .endr
.size native_el0_vectors, .-native_el0_vectors

.macro cyclic_final_stamp
    .ifdef NATIVE_CYCLIC
    stp x16,x17,[sp,#-16]!
    adrp x16,native_cyclic_return_cell
    ldr x16,[x16,:lo12:native_cyclic_return_cell]
    cbz x16,1f
    isb
    mrs x17,cntpct_el0
    str x17,[x16]
1:  ldp x16,x17,[sp],#16
    .endif
.endm

.ifdef NATIVE_SERVER
// IRQs remain masked. A rejected frame still lives at SP and has never
// executed; enter Rust with the sampled crossing timestamp in x17.
.macro server_gate retry
    adrp x16,native_server_compare
    ldr x16,[x16,:lo12:native_server_compare]
    isb
    mrs x17,cntpct_el0
    cmp x17,x16
    b.hs \retry
    adrp x16,native_server_last_return
    str x17,[x16,:lo12:native_server_last_return]
.endm

.global native_server_enter
native_server_enter:
    // Use exactly the normal frame restore, including on the first entry.
    .ifdef NATIVE_AMC
    sub sp,sp,#16
    .endif
    sub sp,sp,#832
    mov x1,sp
    mov x2,#52
1:  ldp x3,x4,[x0],#16
    stp x3,x4,[x1],#16
    subs x2,x2,#1
    b.ne 1b
    b .Lrestore
.endif

.ifdef NATIVE_WEIGHTED_CONTINUOUS
// Written by Rust while IRQs are masked; read back at the next exception.
// Each field is one u64. No pending flag means preserve the existing timer
// and its associated launch evidence across this return.
.pushsection .bss.native_weighted_arm,"aw",%nobits
.balign 8
.global native_weighted_arm
.type native_weighted_arm,%object
native_weighted_arm:
    .zero 80
.size native_weighted_arm,.-native_weighted_arm
.popsection
.endif

.macro weighted_final_arm
    .ifdef NATIVE_WEIGHTED_CONTINUOUS
    stp x12,x13,[sp,#-32]!
    stp x14,x15,[sp,#16]
    adrp x12,native_weighted_arm
    add x12,x12,:lo12:native_weighted_arm
    ldr x13,[x12]                 // pending
    cbz x13,.Lweighted_arm_done\@
    ldp x14,x15,[x12,#8]          // quantum, earliest wake (MAX = none)
    isb
    mrs x13,cntpct_el0            // R is the operand of the actual timer arm
    str x13,[x12,#24]
    adds x14,x13,x14
    b.cs .Lweighted_arm_overflow\@
    cmp x15,x14
    csel x14,x15,x14,lo
    msr cntp_cval_el0,x14
    mov x13,#1
    msr cntp_ctl_el0,x13
    isb
    mrs x13,cntp_cval_el0
    mrs x14,cntp_ctl_el0
    stp x13,x14,[x12,#32]
    .ifdef NATIVE_PROPORTIONAL
    adr x13,90f
    .else
    adr x13,.Lweighted_eret\@
    .endif
    mrs x14,ttbr0_el1
    stp x13,x14,[x12,#56]
    str xzr,[x12]
    isb
    mrs x13,cntpct_el0
    str x13,[x12,#48]
.Lweighted_arm_done\@:
    ldp x14,x15,[sp,#16]
    ldp x12,x13,[sp],#32
    b .Lweighted_eret\@
.Lweighted_arm_overflow\@:
    mov x13,#1
    str x13,[x12,#72]
    str xzr,[x12]
    msr cntp_ctl_el0,xzr
    isb
    b native_weighted_arm_overflow
.Lweighted_eret\@:
    .endif
.endm

.ifdef NATIVE_BATCH
.global native_batch_enter
native_batch_enter:
    sub sp,sp,#848
    mov x1,sp
    mov x2,#52
1:  ldp x3,x4,[x0],#16
    stp x3,x4,[x1],#16
    subs x2,x2,#1
    b.ne 1b
    b .Lrestore
.endif

.global native_el0_enter
.type native_el0_enter,%function
native_el0_enter:
    msr daifset,#15
    mov x19,x4 // retain envp across measured_return (AAPCS callee-saved)
    msr tpidrro_el0,x3
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
    .ifdef NATIVE_MEASURED
    measured_return
    .endif
    msr tpidr_el0,xzr
    msr fpcr,xzr
    msr fpsr,xzr
    .irp reg,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
        movi v\reg\().16b,#0
    .endr
    mov x2,x19
    .irp reg,0,1,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30
        mov x\reg,xzr
    .endr
    clrex
    isb
    .ifdef NATIVE_PROPORTIONAL
    weighted_final_arm
    cyclic_final_stamp
90:
    .else
    cyclic_final_stamp
    weighted_final_arm
    .endif
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
    .ifdef NATIVE_EARLY_ENTRY
    ldr x2,[sp,#840]
    .else
    .ifdef NATIVE_MEASURED
    isb
    mrs x2,cntpct_el0
    .endif
    .endif
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
    .ifdef NATIVE_MEASURED
    mov x1,x2
    .endif
    mov x0,sp
    bl native_sleep_irq
    b .Lrestore

.Luser_sync:
    save_native_frame
    mov x0,sp
    mov x1,#0x400
    bl native_el0_sync
.Lrestore:
    .ifdef NATIVE_MEASURED
    measured_return
    .endif
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
    clrex
    .ifdef NATIVE_SERVER
    .ifdef NATIVE_AMC
    // Arm from the final return sample so kernel residency cannot consume
    // the new entitlement. The absolute calendar may cut the slice shorter.
    adrp x16,native_server_compare
    ldr x16,[x16,:lo12:native_server_compare]
    isb
    mrs x17,cntpct_el0
    cmp x17,x16
    b.hs .Lserver_user_retry
    adrp x15,native_amc_remaining
    ldr x15,[x15,:lo12:native_amc_remaining]
    adds x15,x17,x15
    b.cs .Lunexpected
    cmp x15,x16
    csel x16,x15,x16,lo
    msr cntp_cval_el0,x16
    mov x15,#1
    msr cntp_ctl_el0,x15
    isb
    adrp x15,native_server_last_return
    str x17,[x15,:lo12:native_server_last_return]
    .ifdef NATIVE_AMC_CLOCK_PAGE
    // x17 is the very same R used to arm the physical compare above.
    // Only the admitted EL0 return reaches this point, with IRQs masked.
    adrp x15,native_amc_clock_alias
    ldr x15,[x15,:lo12:native_amc_clock_alias]
    cbz x15,.Lamc_page_done
    str x17,[x15,#56]
    ldr x16,[x15]
    add x16,x16,#1
    stlr x16,[x15]
    adrp x15,native_amc_clock_audit
    ldr x15,[x15,:lo12:native_amc_clock_audit]
    cbz x15,.Lamc_page_done
    stp x16,x17,[x15]
.Lamc_page_done:
    .endif
    ldr x15,[sp,#120]
    .else
    server_gate .Lserver_user_retry
    .endif
    ldp x16,x17,[sp,#128]
    .ifdef NATIVE_AMC
    add sp,sp,#848
    .else
    add sp,sp,#832
    .endif
    eret
.Lserver_user_retry:
    mov x0,x17
    mov x1,sp
    bl native_server_reselect
    b .Lrestore
    .else
    .ifdef NATIVE_PROPORTIONAL
    adrp x16,native_prop_continuation_deadline
    ldr x16,[x16,:lo12:native_prop_continuation_deadline]
    cbz x16,.Lprop_return_ok
    isb
    mrs x17,cntpct_el0
    cmp x17,x16
    b.lo .Lprop_return_ok
    mov x0,x17
    mov x1,sp
    bl native_prop_late
    b .Lrestore
.Lprop_return_ok:
    ldp x16,x17,[sp,#128]
    .endif
    .ifdef NATIVE_BATCH
    adrp x16,native_batch_next_compare
    ldr x16,[x16,:lo12:native_batch_next_compare]
    cbz x16,.Lbatch_return_ok
    isb
    mrs x17,cntpct_el0
    cmp x17,x16
    b.lo .Lbatch_return_ok
    mov x0,x17
    mov x1,sp
    bl native_batch_recheck
    b .Lrestore
.Lbatch_return_ok:
    ldp x16,x17,[sp,#128]
    .endif
    .ifdef NATIVE_EARLY_ENTRY
    add sp,sp,#848
    .else
    add sp,sp,#832
    .endif
    .ifdef NATIVE_PROPORTIONAL
    weighted_final_arm
    cyclic_final_stamp
90:
    .else
    cyclic_final_stamp
    weighted_final_arm
    .endif
    eret
    .endif

// Keep the calling Rust continuation and its NativeFrame on the process
// stack. x19 holds the process SP throughout idle (AAPCS callee-saved).
.global native_sleep_switch
.type native_sleep_switch,%function
native_sleep_switch:
    stp x19,x30,[sp,#-16]!
    mov x19,sp
    mov sp,x0
    .ifdef NATIVE_CYCLIC
 .Lidle_prepare:
    bl native_cyclic_return
    .ifdef NATIVE_SERVER
    server_gate .Lserver_idle_retry
    .else
    cyclic_final_stamp
    .endif
    b native_cyclic_idle_landing
    .ifdef NATIVE_SERVER
.Lserver_idle_retry:
    mov x0,x17
    mov x1,xzr
    bl native_server_reselect
    cbnz x0,.Lleave_idle
    b .Lidle_prepare
    .endif
.global native_cyclic_idle_landing
native_cyclic_idle_landing:
    isb
    mrs x0,cntpct_el0
    bl native_cyclic_idle_sample
    .ifdef NATIVE_EXTENSION
    bl native_extension_on_idle
    .endif
    .endif
    .ifdef NATIVE_HIERARCHY
    // The previously sampled landing precedes this masked boundary. No
    // instrumentation executes between DAIFClr and the existing ISB/WFI.
    adrp x0,native_hierarchy_pre_unmask
    add x0,x0,:lo12:native_hierarchy_pre_unmask
    mrs x2,daif
    isb
    mrs x1,cntpct_el0
    stp x1,x2,[x0]
    .endif
    msr daifclr,#2
    isb
.global native_sleep_idle
native_sleep_idle:
    wfi
    b native_sleep_idle
.size native_sleep_switch,.-native_sleep_switch

    .ifdef NATIVE_EXTENSION
// On interruption discard the evaluator continuation and recover the idle
// caller's AAPCS state. Rust never keeps a trusted mutable borrow unmasked.
.macro extension_restore
    ldp d8,d9,[sp,#96]
    ldp d10,d11,[sp,#112]
    ldp d12,d13,[sp,#128]
    ldp d14,d15,[sp,#144]
    ldp x19,x20,[sp,#0]
    ldp x21,x22,[sp,#16]
    ldp x23,x24,[sp,#32]
    ldp x25,x26,[sp,#48]
    ldp x27,x28,[sp,#64]
    ldp x29,x30,[sp,#80]
    add sp,sp,#160
.endm
.macro extension_abort_stack
    adrp x10,native_extension_caller
    ldr x11,[x10,:lo12:native_extension_caller]
    cbz x11,1f
    str xzr,[x10,:lo12:native_extension_caller]
    mov sp,x11
    extension_restore
1:
.endm
native_extension_on_idle:
    stp x19,x30,[sp,#-16]!
    bl native_extension_stack_if_pending
    ldp x19,x30,[sp],#16
    cbnz x0,native_extension_eval_stack
    ret
native_extension_eval_stack:
    sub sp,sp,#160
    stp x19,x20,[sp,#0]
    stp x21,x22,[sp,#16]
    stp x23,x24,[sp,#32]
    stp x25,x26,[sp,#48]
    stp x27,x28,[sp,#64]
    stp x29,x30,[sp,#80]
    stp d8,d9,[sp,#96]
    stp d10,d11,[sp,#112]
    stp d12,d13,[sp,#128]
    stp d14,d15,[sp,#144]
    mov x11,sp
    adrp x10,native_extension_caller
    str x11,[x10,:lo12:native_extension_caller]
    mov sp,x0
    bl native_extension_evaluate
    extension_abort_stack
    ret
    .endif
.Lidle_irq:
    .ifdef NATIVE_EXTENSION
    extension_abort_stack
    .endif
    // IRQ delivery at EL1h is enabled only inside the assembly idle loop.
    // Its only live value is x19. IRQ entry masked delivery before this call.
    mov x0,xzr
    .ifndef NATIVE_EARLY_ENTRY
    .ifdef NATIVE_MEASURED
    isb
    mrs x1,cntpct_el0
    .endif
    .endif
    bl native_sleep_irq
    .ifdef NATIVE_CYCLIC
    cbnz x0,.Lcyclic_leave
.Lidle_eret_prepare:
    adr x9,native_cyclic_idle_landing
    msr elr_el1,x9
    mrs x9,spsr_el1
    orr x9,x9,#0x80
    msr spsr_el1,x9
    bl native_cyclic_return
    .ifdef NATIVE_SERVER
    server_gate .Lserver_idle_eret_retry
    eret
.Lserver_idle_eret_retry:
    mov x0,x17
    mov x1,xzr
    bl native_server_reselect
    cbnz x0,.Lcyclic_leave
    b .Lidle_eret_prepare
    .else
    cyclic_final_stamp
    b .Lidle_eret
    .endif
.Lcyclic_leave:
    .else
    cbz x0,.Lidle_eret
    .endif
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
