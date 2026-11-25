#ifndef JNOX_LOAN_H
#define JNOX_LOAN_H
#include <stdint.h>
#include <stddef.h>

#define JNOX_LOAN_PAGE_BYTES 4096
typedef struct { long status; uint64_t handle; volatile unsigned char *page; } jnox_writer_loan;
typedef struct {
    long status; uint64_t handle; const volatile unsigned char *page; size_t length;
} jnox_reader_loan;
typedef struct { long status; uint64_t handle, va, length; } jnox_loan_registers;

static inline jnox_loan_registers jnox_loan_trap(uint64_t op, uint64_t a, uint64_t b) {
    register uint64_t x0 __asm__("x0") = a;
    register uint64_t x1 __asm__("x1") = b;
    register uint64_t x2 __asm__("x2");
    register uint64_t x3 __asm__("x3");
    register uint64_t x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "=r"(x2), "=r"(x3)
                     : "r"(x8) : "memory", "cc");
    return (jnox_loan_registers){(long)x0, x1, x2, x3};
}
/* Page pointers are valid only during ownership of a successful grant.
 * Publication, release or channel revocation can invalidate the mapping. */
static inline jnox_writer_loan jnox_loan_acquire(void) {
    jnox_loan_registers r = jnox_loan_trap(33, 0, 0);
    return (jnox_writer_loan){r.status, r.handle, (volatile unsigned char *)r.va};
}
static inline long jnox_loan_publish(uint64_t handle, size_t length) {
    return jnox_loan_trap(34, handle, length).status;
}
static inline jnox_reader_loan jnox_loan_receive(void) {
    jnox_loan_registers r = jnox_loan_trap(35, 0, 0);
    return (jnox_reader_loan){r.status, r.handle, (const volatile unsigned char *)r.va, r.length};
}
static inline long jnox_loan_release(uint64_t handle) {
    return jnox_loan_trap(36, handle, 0).status;
}
#endif
