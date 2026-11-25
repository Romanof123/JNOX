#ifndef JNOX_PTHREAD_H
#define JNOX_PTHREAD_H
#include <stddef.h>
#include <stdint.h>

typedef uint64_t pthread_t;
typedef struct { uint64_t opaque; } pthread_attr_t;

/* NULL selects default attributes. Errors are returned as positive numbers. */
int pthread_create(pthread_t *thread, const pthread_attr_t *attr,
                   void *(*start)(void *), void *arg);
int pthread_join(pthread_t thread, void **result);
_Noreturn void pthread_exit(void *value);

typedef struct { _Alignas(8) uint64_t opaque; } pthread_mutex_t;
typedef struct { uint64_t opaque; } pthread_mutexattr_t;

/* Explicitly initialize at a stable address with NULL attributes. Serialize
 * init/destroy against every operation. These calls return errno directly. */
int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr);
int pthread_mutex_destroy(pthread_mutex_t *mutex);
int pthread_mutex_lock(pthread_mutex_t *mutex);
int pthread_mutex_trylock(pthread_mutex_t *mutex);
int pthread_mutex_unlock(pthread_mutex_t *mutex);
#endif
