#ifndef JNOX_IPC_H
#define JNOX_IPC_H
#include <stdint.h>

/* Raw signed status (zero on success). Only a successful operation has
 * output words. Deadlines use absolute monotonic ns; UINT64_MAX waits. */
typedef struct { long status; uint64_t word; } jnox_ipc_response;
typedef struct { long status; uint64_t token, word; } jnox_ipc_request;

typedef struct { long status; uint64_t x1, x2; } jnox_ipc_registers;

static inline jnox_ipc_registers jnox_ipc_trap(uint64_t op, uint64_t a,
                                              uint64_t b, uint64_t c) {
    register uint64_t x0 __asm__("x0") = a;
    register uint64_t x1 __asm__("x1") = b;
    register uint64_t x2 __asm__("x2") = c;
    register uint64_t x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2)
                     : "r"(x8) : "memory", "cc");
    return (jnox_ipc_registers){(long)x0, x1, x2};
}

static inline jnox_ipc_response jnox_ipc_call(uint64_t server, uint64_t word,
                                              uint64_t deadline_ns) {
    jnox_ipc_registers r = jnox_ipc_trap(21, server, word, deadline_ns);
    return (jnox_ipc_response){r.status, r.x1};
}
static inline jnox_ipc_request jnox_ipc_receive(void) {
    jnox_ipc_registers r = jnox_ipc_trap(22, 0, 0, 0);
    return (jnox_ipc_request){r.status, r.x1, r.x2};
}
static inline long jnox_ipc_reply(uint64_t token, uint64_t word) {
    return jnox_ipc_trap(23, token, word, 0).status;
}
#endif
