#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

/* This known date is an independent test input to the package builder. */
#define TEST_EPOCH 2208988800LL

static int later(time_t sec, long usec, time_t other_sec, long other_usec)
{
    return sec > other_sec || (sec == other_sec && usec > other_usec);
}

static int sample(void)
{
    struct timespec before, after;
    struct timeval wall = {-7, 321};
    if (clock_gettime(CLOCK_MONOTONIC, &before) != 0) return 121;
    errno = 0;
    int status = gettimeofday(&wall, NULL);
    int error = errno;
    if (clock_gettime(CLOCK_MONOTONIC, &after) != 0 ||
        later(before.tv_sec, before.tv_nsec, after.tv_sec, after.tv_nsec)) return 122;
#ifdef JNOX_TEST_UTC_SEEDED
    if (status != 0 || wall.tv_sec < TEST_EPOCH ||
        wall.tv_usec < 0 || wall.tv_usec >= 1000000) return 123;
    time_t elapsed = wall.tv_sec - TEST_EPOCH;
    if (later(before.tv_sec, before.tv_nsec / 1000, elapsed, wall.tv_usec) ||
        later(elapsed, wall.tv_usec, after.tv_sec, after.tv_nsec / 1000)) return 124;
#else
    if (status != -1 || error != ENODATA || wall.tv_sec != -7 || wall.tv_usec != 321) return 125;
#endif
    return 0;
}

int main(void)
{
    int result = sample();
    if (result) return result;
    struct timeval unchanged = {-9, 42};
    errno = 0;
    if (gettimeofday(&unchanged, &result) != -1 || errno != ENOTSUP ||
        unchanged.tv_sec != -9 || unchanged.tv_usec != 42) return 126;
    errno = 0;
    if (gettimeofday(NULL, NULL) != -1 || errno != EFAULT) return 127;
#ifdef JNOX_TEST_UTC_SEEDED
    errno = 0;
    if (gettimeofday((struct timeval *)(uintptr_t)1, NULL) != -1 || errno != EFAULT) return 128;
#endif
    struct timespec start, end;
    if (clock_gettime(CLOCK_MONOTONIC, &start) != 0 || sleep(1) != 0 ||
        clock_gettime(CLOCK_MONOTONIC, &end) != 0 ||
        later(start.tv_sec + 1, start.tv_nsec, end.tv_sec, end.tv_nsec)) return 129;
    result = sample();
    if (result) return result;
    return 87;
}
