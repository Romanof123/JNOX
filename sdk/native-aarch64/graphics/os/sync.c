/* Process-private default mutex and condition operations used by Wayland.
 * Contention sleeps on the JNOX monotonic clock for one millisecond. */
#include "pthread.h"
#include <errno.h>
int jnox_graphics_pause_waiter(void);

int pthread_mutex_init(pthread_mutex_t *m, const pthread_mutexattr_t *attr)
{
    if (attr) return ENOTSUP;
    m->locked = 0;
    return 0;
}
int pthread_mutex_lock(pthread_mutex_t *m)
{
    while (__atomic_exchange_n(&m->locked, 1, __ATOMIC_ACQUIRE)) {
        int r = jnox_graphics_pause_waiter();
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
/* A waiter owns its node until it has unregistered and reacquired its mutex.
 * All list accesses and notification flags are serialized by this guard. */
struct jnox_cond_waiter {
    struct jnox_cond_waiter *next;
    unsigned notified;
};

static void cond_lock(pthread_cond_t *c)
{
    while (__atomic_exchange_n(&c->guard, 1, __ATOMIC_ACQUIRE)) { }
}

static void cond_unlock(pthread_cond_t *c)
{
    __atomic_store_n(&c->guard, 0, __ATOMIC_RELEASE);
}

int pthread_cond_destroy(pthread_cond_t *c)
{
    cond_lock(c);
    int r = c->head ? EBUSY : 0;
    cond_unlock(c);
    return r;
}

int pthread_cond_signal(pthread_cond_t *c)
{
    cond_lock(c);
    for (struct jnox_cond_waiter *w = c->head; w; w = w->next) {
        if (!w->notified) {
            w->notified = 1;
            break;
        }
    }
    cond_unlock(c);
    return 0;
}

int pthread_cond_broadcast(pthread_cond_t *c)
{
    cond_lock(c);
    for (struct jnox_cond_waiter *w = c->head; w; w = w->next)
        w->notified = 1;
    cond_unlock(c);
    return 0;
}

int pthread_cond_wait(pthread_cond_t *c, pthread_mutex_t *m)
{
    struct jnox_cond_waiter waiter = {0};
    cond_lock(c);
    waiter.next = c->head;
    c->head = &waiter;
    pthread_mutex_unlock(m);
    cond_unlock(c);

    int r = 0;
    cond_lock(c);
    while (!waiter.notified && !r) {
        cond_unlock(c);
        r = jnox_graphics_pause_waiter();
        cond_lock(c);
    }
    cond_unlock(c);
    int lock_result = pthread_mutex_lock(m);
    cond_lock(c);
    struct jnox_cond_waiter **link = &c->head;
    while (*link != &waiter) link = &(*link)->next;
    *link = waiter.next;
    cond_unlock(c);
    return lock_result ? lock_result : r;
}
