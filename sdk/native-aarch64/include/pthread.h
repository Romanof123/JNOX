#ifndef JNOX_PTHREAD_H
#define JNOX_PTHREAD_H
#include <stddef.h>
#include <stdint.h>
#include <sched.h>
#include <time.h>

typedef uint64_t pthread_t;
#define PTHREAD_INHERIT_SCHED 0
#define PTHREAD_EXPLICIT_SCHED 1
#define PTHREAD_PRIO_NONE 0
#define PTHREAD_PRIO_INHERIT 1
#define PTHREAD_PRIO_PROTECT 2

typedef struct {
    _Alignas(8) uint32_t tag;
    unsigned char policy, priority, explicit_sched, reserved;
} pthread_attr_t;
int pthread_attr_init(pthread_attr_t *attr);
int pthread_attr_destroy(pthread_attr_t *attr);
int pthread_attr_setschedpolicy(pthread_attr_t *attr, int policy);
int pthread_attr_setschedparam(pthread_attr_t *attr, const struct sched_param *param);
int pthread_attr_setinheritsched(pthread_attr_t *attr, int inherit);

/* NULL selects default attributes. Errors are returned as positive numbers. */
int pthread_create(pthread_t *thread, const pthread_attr_t *attr,
                   void *(*start)(void *), void *arg);
int pthread_join(pthread_t thread, void **result);
_Noreturn void pthread_exit(void *value);

typedef struct { _Alignas(8) uint64_t opaque; } pthread_mutex_t;
typedef struct {
    _Alignas(8) uint32_t tag;
    unsigned char protocol, users, ceiling, reserved;
} pthread_mutexattr_t;

int pthread_mutexattr_init(pthread_mutexattr_t *attr);
int pthread_mutexattr_destroy(pthread_mutexattr_t *attr);
int pthread_mutexattr_setprotocol(pthread_mutexattr_t *attr, int protocol);
int pthread_mutexattr_setprioceiling(pthread_mutexattr_t *attr, int ceiling);
int jnox_mutexattr_setusers(pthread_mutexattr_t *attr, uint32_t mask);

/* Explicitly initialize at a stable address with default or configured attributes. Serialize
 * init/destroy against every operation. These calls return errno directly. */
int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr);
int pthread_mutex_destroy(pthread_mutex_t *mutex);
int pthread_mutex_lock(pthread_mutex_t *mutex);
int pthread_mutex_clocklock(pthread_mutex_t *mutex, clockid_t clock, const struct timespec *absolute);
int pthread_mutex_trylock(pthread_mutex_t *mutex);
int pthread_mutex_unlock(pthread_mutex_t *mutex);
#endif
