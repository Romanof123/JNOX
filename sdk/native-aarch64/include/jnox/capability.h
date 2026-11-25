#ifndef JNOX_CAPABILITY_H
#define JNOX_CAPABILITY_H
#include <errno.h>
#include <stdint.h>

#define JNOX_CAP_SEND 1u
#define JNOX_CAP_DELEGATE 2u

typedef struct { uint64_t raw; } jnox_cap_handle;
typedef struct {
    jnox_cap_handle handle;
    uint64_t tag, sender, target, rights;
} jnox_cap_notice;
typedef struct {
    uint64_t handles, notices, peak_handles, peak_notices;
} jnox_cap_counts;

struct jnox_cap_registers { long status; uint64_t r1, r2, r3, r4, r5; };
static inline struct jnox_cap_registers jnox_cap_trap(
    uint64_t op, uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e) {
    register uint64_t x0 __asm__("x0") = a;
    register uint64_t x1 __asm__("x1") = b;
    register uint64_t x2 __asm__("x2") = c;
    register uint64_t x3 __asm__("x3") = d;
    register uint64_t x4 __asm__("x4") = e;
    register uint64_t x5 __asm__("x5") = 0;
    register uint64_t x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01"
                     : "+r"(x0), "+r"(x1), "+r"(x2), "+r"(x3), "+r"(x4), "+r"(x5)
                     : "r"(x8) : "memory", "cc");
    return (struct jnox_cap_registers){(long)x0, x1, x2, x3, x4, x5};
}

/* Zero or negative errno. Output objects are written only on success. */
static inline long jnox_cap_initial(uint64_t index, jnox_cap_handle *out) {
    if (!out) return -EFAULT;
    struct jnox_cap_registers r = jnox_cap_trap(53, index, 0, 0, 0, 0);
    if (!r.status) out->raw = r.r1;
    return r.status;
}
static inline long jnox_cap_copy(jnox_cap_handle h, uint64_t dst, uint64_t rights, uint64_t tag) {
    return jnox_cap_trap(54, h.raw, dst, rights, 0, tag).status;
}
static inline long jnox_cap_move(jnox_cap_handle h, uint64_t dst, uint64_t rights, uint64_t tag) {
    return jnox_cap_trap(54, h.raw, dst, rights, 1, tag).status;
}
static inline long jnox_cap_receive(jnox_cap_notice *out) {
    if (!out) return -EFAULT;
    struct jnox_cap_registers r = jnox_cap_trap(55, 0, 0, 0, 0, 0);
    if (!r.status) *out = (jnox_cap_notice){{r.r1}, r.r2, r.r3, r.r4, r.r5};
    return r.status;
}
static inline long jnox_cap_send(jnox_cap_handle h, uint64_t word, uint64_t deadline_ns) {
    return jnox_cap_trap(56, h.raw, word, deadline_ns, 0, 0).status;
}
static inline long jnox_cap_close(jnox_cap_handle h) {
    return jnox_cap_trap(57, h.raw, 0, 0, 0, 0).status;
}
static inline long jnox_cap_observe(jnox_cap_counts *out) {
    if (!out) return -EFAULT;
    struct jnox_cap_registers r = jnox_cap_trap(58, 0, 0, 0, 0, 0);
    if (!r.status) *out = (jnox_cap_counts){r.r1, r.r2, r.r3, r.r4};
    return r.status;
}
#endif
