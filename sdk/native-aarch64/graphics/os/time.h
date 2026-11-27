#ifndef JNOX_GRAPHICS_TIME_H
#define JNOX_GRAPHICS_TIME_H
/* Advertise the implemented clock to Picolibc's own header. */
#ifndef _POSIX_MONOTONIC_CLOCK
#define _POSIX_MONOTONIC_CLOCK 200809L
#endif
#include_next <time.h>
#if __POSIX_VISIBLE
int clock_gettime(clockid_t clock, struct timespec *result);
int clock_getres(clockid_t clock, struct timespec *result);
#endif
#endif
