#ifndef JNOX_CLOCK_PAIR_H
#define JNOX_CLOCK_PAIR_H
#include <stdint.h>

struct jnox_clock_pair {
    uint64_t monotonic_ns, process_cpu_ns;
    uint32_t backend, error;
};

/* Backend 1 samples both clocks at one counter read. Backend 2 samples CPU
 * then monotonic using two clock syscalls. Backend 0 carries positive errno. */
struct jnox_clock_pair jnox_clock_sample(void);
#endif
