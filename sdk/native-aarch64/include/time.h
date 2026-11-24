#ifndef JNOX_TIME_H
#define JNOX_TIME_H

typedef long time_t;
typedef int clockid_t;
struct timespec {
    time_t tv_sec;
    long tv_nsec;
};

#define CLOCK_MONOTONIC 1

int clock_gettime(clockid_t clock, struct timespec *result);
#endif
