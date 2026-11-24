#include <stdint.h>
#include <time.h>

static int valid(const struct timespec *time)
{
    return time->tv_sec >= 0 && time->tv_nsec >= 0 && time->tv_nsec < 1000000000L;
}

int main(void)
{
    static const volatile unsigned char input[] = "Wikipedia";
    struct timespec start, end;
    unsigned int a = 1, b = 0;
    if (clock_gettime(CLOCK_MONOTONIC, &start) || !valid(&start))
        return 1;
    for (unsigned int i = 0; i < sizeof(input) - 1; ++i) {
        a = (a + input[i]) % 65521;
        b = (b + a) % 65521;
    }
    if (clock_gettime(CLOCK_MONOTONIC, &end) || !valid(&end))
        return 2;
    if (end.tv_sec < start.tv_sec ||
        (end.tv_sec == start.tv_sec && end.tv_nsec < start.tv_nsec))
        return 3;
    return ((b << 16) | a) == 0x11e60398u ? 0 : 4;
}