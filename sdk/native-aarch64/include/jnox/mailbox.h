#ifndef JNOX_MAILBOX_H
#define JNOX_MAILBOX_H
#include <stdint.h>

/* Absolute monotonic ns. Zero tries immediately; UINT64_MAX waits indefinitely.
 * Status is the signed kernel result. Only successful receives return a word. */
typedef struct { long status; uint64_t word; } jnox_mailbox_result;

static inline long jnox_mailbox_send(uint64_t owner, uint64_t word, uint64_t deadline_ns) {
    register uint64_t x0 __asm__("x0") = owner;
    register uint64_t x1 __asm__("x1") = word;
    register uint64_t x2 __asm__("x2") = deadline_ns;
    register uint64_t x8 __asm__("x8") = 24;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2)
                     : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static inline jnox_mailbox_result jnox_mailbox_receive(uint64_t deadline_ns) {
    register uint64_t x0 __asm__("x0") = deadline_ns;
    register uint64_t x1 __asm__("x1");
    register uint64_t x8 __asm__("x8") = 25;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "=r"(x1) : "r"(x8) : "x2", "memory", "cc");
    return (jnox_mailbox_result){(long)x0, x1};
}
#endif
