#ifndef JNOX_GRAPHICS_PTHREAD_H
#define JNOX_GRAPHICS_PTHREAD_H
#include <stdint.h>
#include <time.h>
/* X11's public locking records use these handle types even in the
 * --disable-xthreads build. pthread_t matches the native AArch64 SDK ABI. */
typedef uint64_t pthread_t;
typedef unsigned int pthread_key_t;
typedef struct { unsigned locked; } pthread_mutex_t;
struct jnox_cond_waiter;
typedef struct { struct jnox_cond_waiter *head; unsigned guard; } pthread_cond_t;
typedef struct { unsigned reserved; } pthread_mutexattr_t;
typedef struct { unsigned reserved; } pthread_condattr_t;
#define PTHREAD_MUTEX_INITIALIZER {0}
#define PTHREAD_COND_INITIALIZER {0}
int pthread_mutex_init(pthread_mutex_t *, const pthread_mutexattr_t *);
int pthread_mutex_destroy(pthread_mutex_t *);
int pthread_mutex_lock(pthread_mutex_t *);
int pthread_mutex_unlock(pthread_mutex_t *);
int pthread_cond_init(pthread_cond_t *, const pthread_condattr_t *);
int pthread_cond_destroy(pthread_cond_t *);
int pthread_cond_wait(pthread_cond_t *, pthread_mutex_t *);
int pthread_cond_signal(pthread_cond_t *);
int pthread_cond_broadcast(pthread_cond_t *);
#endif
