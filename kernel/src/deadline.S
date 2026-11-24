.macro deadline_report id, stage
    mov x0, #\id
    mov x1, #\stage
    mov x2, sp
    bl {marker}
.endm

.macro deadline_enter id
    sub sp, sp, #16
    stp x19, xzr, [sp]
    deadline_report \id, 0
    // Quick jobs overwrite the outgoing worker's TLS and SIMD witnesses.
    msr tpidr_el1, x19
    dup v31.2d, x19
.endm

// X22 is the next logical tick. The kernel has already programmed its fixed
// physical-counter boundary. Unmask only after this tick's witnesses are live.
.macro deadline_work begin, end
    msr daifset, #2
    add x21, x19, x22
    lsl x20, x21, #32
    msr tpidr_el1, x21
    dup v31.2d, x21
    str xzr, [sp, #8]
    adrp x12, {tick}
    add x12, x12, :lo12:{tick}
    add x20, x20, #1
    msr daifclr, #2
.global \begin
\begin:
    add x20, x20, #1
    ldar x0, [x12]
    cmp x0, x22
    b.lo \begin
.global \end
\end:
    // At most one increment can follow the interrupted instruction. X20's
    // upper half, the saved stack word, TLS and both SIMD lanes identify
    // this task and this tick, including B's two distinct real resumptions.
    ldp x0, x1, [sp]
    cmp x0, x19
    b.ne .Lunexpected
    cbz x1, .Lunexpected
    sub x0, x20, x1
    cmp x0, #1
    b.hi .Lunexpected
    lsr x0, x20, #32
    cmp x0, x21
    b.ne .Lunexpected
    mrs x0, tpidr_el1
    cmp x0, x21
    b.ne .Lunexpected
    umov x0, v31.d[0]
    cmp x0, x21
    b.ne .Lunexpected
    umov x0, v31.d[1]
    cmp x0, x21
    b.ne .Lunexpected
.endm

.global deadline_a_entry
.type deadline_a_entry, %function
deadline_a_entry:
    deadline_enter {a}
    cbz x23, .Ldeadline_a_finish
    mov x22, #1
    deadline_work deadline_edf_a_begin, deadline_edf_a_end
    deadline_report {a}, 1
.Ldeadline_a_finish:
    svc #1
.global deadline_a_after_exit
deadline_a_after_exit:
    b .Lunexpected
.size deadline_a_entry, . - deadline_a_entry

.global deadline_b_entry
.type deadline_b_entry, %function
deadline_b_entry:
    deadline_enter {b}
    cbz x23, .Ldeadline_b_finish
    mov x22, #1
.Ldeadline_retain:
    deadline_work deadline_llf_b_hold_begin, deadline_llf_b_hold_end
    add x22, x22, #1
    cmp x22, #6
    b.lo .Ldeadline_retain
    deadline_report {b}, 1
    deadline_work deadline_llf_b_cross_begin, deadline_llf_b_cross_end
    deadline_report {b}, 2
    mov x22, #7
    deadline_work deadline_llf_b_negative_begin, deadline_llf_b_negative_end
    deadline_report {b}, 3
.Ldeadline_b_finish:
    svc #1
.global deadline_b_after_exit
deadline_b_after_exit:
    b .Lunexpected
.size deadline_b_entry, . - deadline_b_entry

.global deadline_c_entry
.type deadline_c_entry, %function
deadline_c_entry:
    deadline_enter {c}
    svc #1
.global deadline_c_after_exit
deadline_c_after_exit:
    b .Lunexpected
.size deadline_c_entry, . - deadline_c_entry
