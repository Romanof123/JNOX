#ifndef JNOX_WAITSET_H
#define JNOX_WAITSET_H
#include <stdint.h>

enum jnox_waitset_source {
    JNOX_WAITSET_MAILBOX = 1,
    JNOX_WAITSET_PULSE = 2,
    JNOX_WAITSET_EVENT = 3
};
typedef struct { long status; uint64_t handle; } jnox_waitset_registration;
typedef struct { long status; uint64_t handle, ready; } jnox_waitset_result;

static inline jnox_waitset_result jnox_waitset_trap(uint64_t op, uint64_t a,
                                                    uint64_t b, uint64_t c) {
    register uint64_t x0 __asm__("x0") = a;
    register uint64_t x1 __asm__("x1") = b;
    register uint64_t x2 __asm__("x2") = c;
    register uint64_t x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2)
                     : "r"(x8) : "memory", "cc");
    return (jnox_waitset_result){(long)x0, x1, x2};
}

static inline jnox_waitset_registration jnox_waitset_attach(
    uint64_t owner, enum jnox_waitset_source source, uint32_t mask) {
    jnox_waitset_result r = jnox_waitset_trap(30, owner, source, mask);
    return (jnox_waitset_registration){r.status, r.handle};
}
static inline long jnox_waitset_detach(uint64_t handle) {
    return jnox_waitset_trap(31, handle, 0, 0).status;
}
/* Zero polls, UINT64_MAX waits indefinitely, other values are absolute
 * monotonic nanoseconds. A successful wait leaves the source untouched. */
static inline jnox_waitset_result jnox_waitset_wait(uint64_t deadline_ns) {
    return jnox_waitset_trap(32, deadline_ns, 0, 0);
}
#endif
