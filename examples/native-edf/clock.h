#include <stdint.h>
#include <time.h>

static inline int read_ns(uint64_t *value)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now))
        return -1;
    *value = (uint64_t)now.tv_sec * 1000000000 + (uint64_t)now.tv_nsec;
    return 0;
}

static inline int sleep_to(uint64_t ns)
{
    struct timespec deadline = {
        .tv_sec = (time_t)(ns / 1000000000),
        .tv_nsec = (long)(ns % 1000000000),
    };
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, 0);
}
