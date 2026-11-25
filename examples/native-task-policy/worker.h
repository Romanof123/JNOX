#ifndef POLICY_WORKER_H
#define POLICY_WORKER_H
#include <jnox/task_policy.h>

/* Each iteration depends on the previous value and updates private writable data.
 * This function has a finite bound and contains no system calls. */
static volatile uint32_t checksum = 1;
static inline void calculate(unsigned iterations)
{
    for (unsigned i = 0; i < iterations; ++i)
        checksum = checksum * 1664525u + 1013904223u;
}
#endif
