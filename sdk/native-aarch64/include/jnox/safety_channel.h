#ifndef JNOX_SAFETY_CHANNEL_H
#define JNOX_SAFETY_CHANNEL_H
#include <stdint.h>

/* Virtual permit bench. The kernel authorizes supervisor and controller calls
 * by process identity. Time fields are raw counter ticks, not nanoseconds. */
enum jnox_safety_command {
    JNOX_SAFETY_OPEN = 1, JNOX_SAFETY_REARM = 2,
    JNOX_SAFETY_CLOSE = 3, JNOX_SAFETY_MARK = 4
};
enum jnox_safety_flags {
    JNOX_SAFETY_GATE_OPEN = 1, JNOX_SAFETY_ENABLED = 2,
    JNOX_SAFETY_FAULT = 4, JNOX_SAFETY_PENDING_REARM = 8,
    JNOX_SAFETY_WATCHDOG_PRIMED = 16, JNOX_SAFETY_WATCHDOG_FAULT = 32
};
typedef struct {
    long status;
    uint64_t ticks, hz, generation, flags, last_sequence, last_issue_ticks;
} jnox_safety_state;

static inline long jnox_safety_operation(uint64_t op, uint64_t action,
                                         uint64_t generation, uint64_t sequence,
                                         uint64_t issue_ticks, uint64_t permit)
{
    register uint64_t x8 __asm__("x8") = op;
    register uint64_t x0 __asm__("x0") = action;
    register uint64_t x1 __asm__("x1") = generation;
    register uint64_t x2 __asm__("x2") = sequence;
    register uint64_t x3 __asm__("x3") = issue_ticks;
    register uint64_t x4 __asm__("x4") = permit;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2), "+r"(x3), "+r"(x4)
                     : "r"(x8) : "memory", "cc");
    return (long)x0;
}
static inline long jnox_safety_supervise(enum jnox_safety_command command, uint64_t generation)
{
    return jnox_safety_operation(59, command, generation, 0, 0, 0);
}
/* Closed-gate virtual qualification fixture. Only slot 2 can request its fixed frame. */
static inline long jnox_safety_inject_invalid(uint64_t generation)
{
    return jnox_safety_operation(59, 5, generation, 0, 0, 0);
}
static inline long jnox_safety_check(void)
{
    return jnox_safety_operation(60, 1, 0, 0, 0, 0);
}
/* Call only after validating the entire private 44-byte snapshot. */
static inline long jnox_safety_commit(uint64_t generation, uint64_t sequence,
                                      uint64_t issue_ticks, uint64_t permit)
{
    return jnox_safety_operation(60, 2, generation, sequence, issue_ticks, permit);
}
static inline long jnox_safety_complete(void)
{
    return jnox_safety_operation(60, 3, 0, 0, 0, 0);
}
static inline long jnox_safety_process_rearm(void)
{
    return jnox_safety_operation(60, 4, 0, 0, 0, 0);
}
static inline jnox_safety_state jnox_safety_info(void)
{
    register uint64_t x8 __asm__("x8") = 61;
    register uint64_t x0 __asm__("x0") = 0;
    register uint64_t x1 __asm__("x1") = 0;
    register uint64_t x2 __asm__("x2") = 0;
    register uint64_t x3 __asm__("x3") = 0;
    register uint64_t x4 __asm__("x4") = 0;
    register uint64_t x5 __asm__("x5") = 0;
    register uint64_t x6 __asm__("x6") = 0;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2), "+r"(x3),
                     "+r"(x4), "+r"(x5), "+r"(x6) : "r"(x8) : "memory", "cc");
    return (jnox_safety_state){(long)x0, x1, x2, x3, x4, x5, x6};
}
#endif
