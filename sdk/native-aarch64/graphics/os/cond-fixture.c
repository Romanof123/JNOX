/* Link with sync.c alone. The pause boundary schedules deterministic nested
 * waits in this process; it models notification ordering, not guest threads. */
#include "pthread.h"
#include <errno.h>

static pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
static pthread_mutex_t outer = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t inner = PTHREAD_MUTEX_INITIALIZER;
static int pauses, failure, broadcast;

int jnox_graphics_pause_waiter(void)
{
    ++pauses;
    if (pthread_cond_destroy(&cond) != EBUSY) failure = 1;
    if (pauses == 1) {
        if (pthread_mutex_lock(&inner) || pthread_cond_wait(&cond, &inner) ||
            pthread_mutex_destroy(&inner) != EBUSY || pthread_mutex_unlock(&inner))
            failure = 2;
        return 0;
    }
    if (pauses == 2)
        return broadcast ? pthread_cond_broadcast(&cond) : pthread_cond_signal(&cond);
    if (pauses == 3 && !broadcast) return EIO;
    failure = 3;
    return EIO;
}

int main(void)
{
    pthread_condattr_t attr = {0};
    if (pthread_cond_init(&cond, &attr) != ENOTSUP) return 1;
    for (broadcast = 0; broadcast <= 1; ++broadcast) {
        pauses = 0;
        if (pthread_cond_signal(&cond) || pthread_mutex_lock(&outer)) return 2;
        int status = pthread_cond_wait(&cond, &outer);
        if (status != (broadcast ? 0 : EIO) || pauses != (broadcast ? 2 : 3)) return 3;
        if (pthread_mutex_destroy(&outer) != EBUSY || pthread_mutex_unlock(&outer) ||
            pthread_cond_destroy(&cond) || failure) return 4;
    }
    return 0;
}
