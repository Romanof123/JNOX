#ifndef JNOX_NOTIFY_H
#define JNOX_NOTIFY_H
#include <stdint.h>

/* Receives use absolute monotonic ns: zero tries once, UINT64_MAX waits.
 * Only status zero supplies result fields. Errors preserve signed x0. */
typedef struct { long status; uint32_t code; uint64_t word; } jnox_pulse_result;
typedef struct { long status; uint32_t bits; } jnox_event_result;
typedef struct { long status; uint64_t x1, x2; } jnox_notify_registers;

static inline jnox_notify_registers jnox_notify_trap(uint64_t op, uint64_t a,
                                                     uint64_t b, uint64_t c) {
    register uint64_t x0 __asm__("x0") = a;
    register uint64_t x1 __asm__("x1") = b;
    register uint64_t x2 __asm__("x2") = c;
    register uint64_t x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2)
                     : "r"(x8) : "memory", "cc");
    return (jnox_notify_registers){(long)x0, x1, x2};
}
static inline long jnox_pulse_send(uint64_t owner, uint32_t code, uint64_t word) {
    return jnox_notify_trap(26, owner, code, word).status;
}
static inline jnox_pulse_result jnox_pulse_recv(uint64_t deadline_ns) {
    jnox_notify_registers r = jnox_notify_trap(27, deadline_ns, 0, 0);
    return (jnox_pulse_result){r.status, (uint32_t)r.x1, r.x2};
}
static inline long jnox_event_set(uint64_t owner, uint32_t mask) {
    return jnox_notify_trap(28, owner, mask, 0).status;
}
static inline jnox_event_result jnox_event_take(uint32_t mask, uint64_t deadline_ns) {
    jnox_notify_registers r = jnox_notify_trap(29, mask, deadline_ns, 0);
    return (jnox_event_result){r.status, (uint32_t)r.x1};
}
#endif
