/* Process-private default mutex and condition operations used by Wayland.
 * Contention sleeps on the JNOX monotonic clock for one millisecond. */
#include "pthread.h"
#include <errno.h>
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

static int pause_waiter(void)
{
    struct { long sec, ns; } now;
    long r = jnox_ipc_trap(JNOX_CALL_CLOCK_GETTIME, 1, (uintptr_t)&now, 0).status;
    if (r) return EIO;
    now.ns += 1000000;
    if (now.ns >= 1000000000) {
        now.ns -= 1000000000;
        ++now.sec;
    }
    r = jnox_ipc_trap(JNOX_CALL_CLOCK_NANOSLEEP, 1, 1, (uintptr_t)&now).status;
    return r == 0 || r == -4 ? 0 : EIO;
}

int pthread_mutex_init(pthread_mutex_t *m, const pthread_mutexattr_t *attr)
{
    if (attr) return ENOTSUP;
    m->locked = 0;
    return 0;
}
int pthread_mutex_lock(pthread_mutex_t *m)
{
    while (__atomic_exchange_n(&m->locked, 1, __ATOMIC_ACQUIRE)) {
        int r = pause_waiter();
        if (r) return r;
    }
    return 0;
}
int pthread_mutex_unlock(pthread_mutex_t *m)
{
    __atomic_store_n(&m->locked, 0, __ATOMIC_RELEASE);
    return 0;
}
int pthread_mutex_destroy(pthread_mutex_t *m)
{
    return __atomic_load_n(&m->locked, __ATOMIC_RELAXED) ? EBUSY : 0;
}
int pthread_cond_init(pthread_cond_t *c, const pthread_condattr_t *attr)
{
    if (attr) return ENOTSUP;
    *c = (pthread_cond_t){0};
    return 0;
}
int pthread_cond_destroy(pthread_cond_t *c)
{
    return __atomic_load_n(&c->waiters, __ATOMIC_RELAXED) ? EBUSY : 0;
}
int pthread_cond_broadcast(pthread_cond_t *c)
{
    __atomic_add_fetch(&c->generation, 1, __ATOMIC_RELEASE);
    return 0;
}
int pthread_cond_wait(pthread_cond_t *c, pthread_mutex_t *m)
{
    uint64_t start = __atomic_load_n(&c->generation, __ATOMIC_ACQUIRE);
    __atomic_add_fetch(&c->waiters, 1, __ATOMIC_RELAXED);
    pthread_mutex_unlock(m);
    int r = 0;
    while (__atomic_load_n(&c->generation, __ATOMIC_ACQUIRE) == start && !r)
        r = pause_waiter();
    int lock_result = pthread_mutex_lock(m);
    __atomic_sub_fetch(&c->waiters, 1, __ATOMIC_RELAXED);
    return lock_result ? lock_result : r;
}
