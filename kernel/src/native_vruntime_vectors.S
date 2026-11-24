// Single-core CHG-088 EL0 jobs share the privileged mappings. The scheduler
// copies each incoming frame before selecting a persistent frame to restore.
.section .text.native_vruntime,"ax",%progbits
.balign 2048
.global native_vruntime_vectors
.type native_vruntime_vectors,%object
.macro vr_slot destination
    b \destination
    .space 124
.endm
native_vruntime_vectors:
    .rept 8
        vr_slot .Lvr_unexpected
    .endr
    vr_slot .Lvr_sync
    vr_slot .Lvr_irq
    .rept 6
        vr_slot .Lvr_unexpected
    .endr
.size native_vruntime_vectors, .-native_vruntime_vectors

// These pairs have identical offsets in the single-process and weighted
// frame layouts. x0/x1, x14–x17 and x30 have separate timing/base roles.
.macro vr_integer_pairs op, base
    \op x2,x3,[\base,#16]
    \op x4,x5,[\base,#32]
    \op x6,x7,[\base,#48]
    \op x8,x9,[\base,#64]
    \op x10,x11,[\base,#80]
    \op x12,x13,[\base,#96]
    \op x18,x19,[\base,#144]
    \op x20,x21,[\base,#160]
    \op x22,x23,[\base,#176]
    \op x24,x25,[\base,#192]
    \op x26,x27,[\base,#208]
    \op x28,x29,[\base,#224]
.endm
.macro vr_simd_pairs op, base
    \op q0,q1,[\base,#304]
    \op q2,q3,[\base,#336]
    \op q4,q5,[\base,#368]
    \op q6,q7,[\base,#400]
    \op q8,q9,[\base,#432]
    \op q10,q11,[\base,#464]
    \op q12,q13,[\base,#496]
    \op q14,q15,[\base,#528]
    \op q16,q17,[\base,#560]
    \op q18,q19,[\base,#592]
    \op q20,q21,[\base,#624]
    \op q22,q23,[\base,#656]
    \op q24,q25,[\base,#688]
    \op q26,q27,[\base,#720]
    \op q28,q29,[\base,#752]
    \op q30,q31,[\base,#784]
.endm

.macro vr_private_stack
    msr spsel,#1
    adrp x9,native_vruntime_stack_top
    add x9,x9,:lo12:native_vruntime_stack_top
    mov sp,x9
.endm

.global native_vruntime_start
.type native_vruntime_start,%function
native_vruntime_start:
    msr daifset,#15
    mrs x9,CurrentEL
    cmp x9,#4
    b.ne .Lvr_park
    vr_private_stack
    adrp x9,native_vruntime_vectors
    add x9,x9,:lo12:native_vruntime_vectors
    msr vbar_el1,x9
    isb
    b .Lvr_restore
.size native_vruntime_start,.-native_vruntime_start

// Save both scratch registers before sampling. x0 holds the entry tick
// and x1 carries the architectural vector offset until the Rust call.
.macro vr_entry offset
    sub sp,sp,#832
    stp x0,x1,[sp]
    isb
    mrs x0,cntpct_el0
    mov x1,#\offset
    b .Lvr_save
.endm
.Lvr_sync:
    vr_entry 0x400
.Lvr_irq:
    vr_entry 0x480
.Lvr_save:
    vr_integer_pairs stp,sp
    stp x14,x15,[sp,#112]
    stp x16,x17,[sp,#128]
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
    vr_simd_pairs stp,sp
    mrs x9,tpidr_el0
    mrs x10,tpidrro_el0
    str x9,[sp,#816]
    str x10,[sp,#824]
    mov x2,x0
    mov x0,sp
    .ifdef JNOX_RETAINED
    vr_private_stack
    .endif
    bl native_vruntime_trap
    .ifndef JNOX_RETAINED
    add sp,sp,#832
    .endif

.Lvr_restore:
    cbz x0,.Lvr_unexpected
    tst x0,#15
    b.ne .Lvr_unexpected
    mov x30,x0
    .ifdef JNOX_RETAINED
    // Record the actual restore base. vr_entry will subtract one frame
    // from this SP_EL1; EL0t continues using its own SP_EL0.
    adrp x9,native_partition_frame_cursor_v1
    add x9,x9,:lo12:native_partition_frame_cursor_v1
    ldr x10,[x9]
    cmp x10,#15
    b.hs .Lvr_unexpected
    adrp x11,native_partition_restore_v1
    add x11,x11,:lo12:native_partition_restore_v1
    str x30,[x11,x10,lsl #3]
    add x10,x10,#1
    str x10,[x9]
    adrp x11,native_partition_traps_v1
    add x11,x11,:lo12:native_partition_traps_v1
    mov x12,#832
    madd x11,x10,x12,x11
    mov sp,x11
    .endif
    ldp x9,x10,[x30,#256]
    and x11,x9,#0x3ff
    cmp x11,#0x340               // EL0t; D/A/F masked, I clear.
    b.ne .Lvr_unexpected
    tst x10,#3
    b.ne .Lvr_unexpected
    msr spsr_el1,x9
    msr elr_el1,x10
    ldr x9,[x30,#272]
    msr sp_el0,x9
    ldp x9,x10,[x30,#288]
    msr fpcr,x9
    msr fpsr,x10
    ldr x9,[x30,#816]
    ldr x10,[x30,#824]
    msr tpidr_el0,x9
    msr tpidrro_el0,x10
    vr_simd_pairs ldp,x30
    vr_integer_pairs ldp,x30
    ldp x0,x1,[x30]
    adrp x16,native_vruntime_return_tick
    add x16,x16,:lo12:native_vruntime_return_tick
    adrp x14,native_vruntime_quantum
    ldr x14,[x14,:lo12:native_vruntime_quantum]
    cbz x14,.Lvr_unexpected
    adrp x15,native_vruntime_deadline
    ldr x15,[x15,:lo12:native_vruntime_deadline]
    clrex
    isb
    mrs x17,cntpct_el0
    .if JNOX_CHECK_WINDOW
    cmp x17,x15                  // A cyclic release must still be current.
    b.hs .Lvr_unexpected
    .endif
    str x17,[x16]
    adds x14,x17,x14
    b.cs .Lvr_unexpected
    cmp x14,x15
    csel x14,x14,x15,ls
    msr cntp_cval_el0,x14
    mov x15,#1
    msr cntp_ctl_el0,x15
    isb
    ldp x14,x15,[x30,#112]
    ldp x16,x17,[x30,#128]
    ldr x30,[x30,#240]
.global native_vruntime_eret
.type native_vruntime_eret,%function
native_vruntime_eret:
    eret
.size native_vruntime_eret,.-native_vruntime_eret

.Lvr_unexpected:
    msr daifset,#15
    vr_private_stack
    bl native_vruntime_unexpected
.Lvr_park:
    wfe
    b .Lvr_park

.section .bss.native_vruntime,"aw",%nobits
.ifdef JNOX_RETAINED
.macro vr_bank name, bytes
    .balign 16
    .global \name
    .type \name,%object
\name:
    .space \bytes
    .size \name,.-\name
.endm
vr_bank native_partition_traps_v1,12480
vr_bank native_partition_resume_v1,12480
vr_bank native_partition_restore_v1,120
vr_bank native_partition_frame_cursor_v1,8
.endif
.balign 16
.global native_vruntime_stack_bottom
.type native_vruntime_stack_bottom,%object
native_vruntime_stack_bottom:
    .space 16384
.global native_vruntime_stack_top
native_vruntime_stack_top:
.size native_vruntime_stack_bottom,.-native_vruntime_stack_bottom
.balign 8
.global native_vruntime_return_tick
.type native_vruntime_return_tick,%object
native_vruntime_return_tick:
    .space 8
.size native_vruntime_return_tick,.-native_vruntime_return_tick
