#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <time.h>
#include <unistd.h>

int main(void)
{
    struct timespec start, end;
    errno = EBUSY;
    if (sleep(0) != 0 || errno != EBUSY) return 101;
    if (clock_gettime(CLOCK_MONOTONIC, &start) != 0) return 102;
    if (sleep(2) != 0 || errno != EBUSY) return 103;
    if (clock_gettime(CLOCK_MONOTONIC, &end) != 0) return 104;
    time_t elapsed = end.tv_sec - start.tv_sec;
    if (elapsed < 2 || (elapsed == 2 && end.tv_nsec < start.tv_nsec)) return 105;
    return 77;
}
