#ifndef NATIVE_WEIGHTED_CLOCK_HELPERS_H
#define NATIVE_WEIGHTED_CLOCK_HELPERS_H
#include <stdint.h>
#include <time.h>

static int read_ns(uint64_t *value)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now))
        return -1;
    *value = (uint64_t)now.tv_sec * 1000000000 + (uint64_t)now.tv_nsec;
    return 0;
}

static int sleep_to_ns(uint64_t target)
{
    struct timespec deadline = {
        .tv_sec = (time_t)(target / 1000000000),
        .tv_nsec = (long)(target % 1000000000),
    };
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, 0);
}
#endif
