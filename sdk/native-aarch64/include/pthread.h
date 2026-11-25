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
#endif
