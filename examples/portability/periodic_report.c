#include <errno.h>
#include <time.h>

int main(void)
{
    struct timespec target, observed;
    struct timespec remaining = {17, 23};
    const struct timespec invalid = {0, 1000000000L};
    volatile unsigned completed = 0;
    errno = EINTR;
    if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &invalid, &remaining) != EINVAL ||
        errno != EINTR || remaining.tv_sec != 17 || remaining.tv_nsec != 23)
        return 1;
    if (clock_gettime(CLOCK_MONOTONIC, &target))
        return 2;
    for (unsigned round = 1; round <= 3; ++round) {
        target.tv_nsec += 40000000L;
        if (target.tv_nsec >= 1000000000L) {
            target.tv_nsec -= 1000000000L;
            if (target.tv_sec == __LONG_MAX__)
                return 3;
            ++target.tv_sec;
        }
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, &remaining) ||
            clock_gettime(CLOCK_MONOTONIC, &observed))
            return 4;
        if (observed.tv_sec < target.tv_sec ||
            (observed.tv_sec == target.tv_sec && observed.tv_nsec < target.tv_nsec))
            return 5;
        completed += round;
        if (round == 1 && clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, &remaining))
            return 6;
    }
    return completed == 6 && errno == EINTR &&
           remaining.tv_sec == 17 && remaining.tv_nsec == 23 ? 0 : 7;
}
