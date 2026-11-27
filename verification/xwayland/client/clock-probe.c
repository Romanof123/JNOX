#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <time.h>

int main(void)
{
    struct timespec first, second;
    if (clock_gettime(CLOCK_MONOTONIC, &first) ||
        clock_gettime(CLOCK_MONOTONIC, &second)) return 1;
    if (first.tv_nsec < 0 || first.tv_nsec >= 1000000000L ||
        second.tv_nsec < 0 || second.tv_nsec >= 1000000000L) return 2;
    if (second.tv_sec < first.tv_sec ||
        (second.tv_sec == first.tv_sec && second.tv_nsec < first.tv_nsec)) return 3;
    errno = 0;
    if (clock_gettime(CLOCK_REALTIME, &second) != -1 || errno != EINVAL) return 4;
    return 0;
}
