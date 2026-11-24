// Worker and idle have separate work PCs and private progress counters. x0 is
// the dispatched request ID. x22 records the ID actually observed in the loop.
.macro runner entry, begin, end
.global \entry
\entry:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    mov x20, #1
    msr daifclr, #2
.global \begin
\begin:
    mov x22, x0
    add x20, x20, #1
    ldr x10, [sp]
    cmp x19, x10
    b.ne .Lunexpected
    mrs x10, tpidr_el1
    cmp x19, x10
    b.ne .Lunexpected
    b \begin
.global \end
\end:
.endm

runner aper_worker, aper_worker_begin, aper_worker_end
runner aper_idle, aper_idle_begin, aper_idle_end

// Each submitted call returns to the producer. It checks the acknowledgement
// before parking with SVC 2. The next release resumes after that instruction.
.macro submit id, work
    mov x0, #\id
    mov x1, #\work
    svc #1
.global aper_reply_\id
aper_reply_\id:
    cmp x0, #(0x100 + \id)
    b.ne .Lunexpected
    svc #2
.global aper_park_\id
aper_park_\id:
.endm
.global aper_producer
aper_producer:
    sub sp, sp, #16
    stp x19, xzr, [sp]
    msr tpidr_el1, x19
    submit 1, 1
    submit 2, 3
    submit 3, 1
    submit 4, 1
    b .Lunexpected
