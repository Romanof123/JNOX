#ifndef JNOX_PERMIT_TRACE_H
#define JNOX_PERMIT_TRACE_H
#include <stdint.h>

/* SVC 62 is present only on the native safety qualification profile.
 * Input target and timestamps are physical counter ticks. */
static inline long jnox_permit_mark(uint64_t job, uint64_t phase, uint64_t stage, uint64_t target) {
    register uint64_t x0 __asm__("x0") = job;
    register uint64_t x1 __asm__("x1") = phase;
    register uint64_t x2 __asm__("x2") = stage;
    register uint64_t x3 __asm__("x3") = target;
    register uint64_t x8 __asm__("x8") = 62;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x1), "r"(x2), "r"(x3), "r"(x8)
                     : "memory", "cc");
    return (long)x0;
}
#endif
