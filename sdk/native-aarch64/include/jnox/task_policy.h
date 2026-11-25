#ifndef JNOX_TASK_POLICY_H
#define JNOX_TASK_POLICY_H
#include <stdint.h>

/* Native AArch64 v1 policy calls preserve the signed status returned in x0. */
static inline long jnox_task_policy_call(unsigned op, long arg)
{
    register long x0 __asm__("x0") = arg;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x8) : "memory", "cc");
    return x0;
}

/* Complete the current job. Returns zero on the next release, or a negative error. */
static inline long jnox_task_complete(void) { return jnox_task_policy_call(80, 0); }
/* Stop this task and record its nonzero fault code. Only errors return. */
static inline long jnox_task_fault(uint32_t code) { return jnox_task_policy_call(81, code); }
/* Stop this task with the supplied exit status. Only errors return. */
static inline long jnox_task_exit(int status) { return jnox_task_policy_call(82, status); }
#endif
