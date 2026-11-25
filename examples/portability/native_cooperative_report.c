#include <sched.h>
#include <time.h>
#include <stdint.h>

/* Both report programs place their accumulator first in the writable page. */
__attribute__((section(".data.private_report")))
volatile unsigned primary_total = 10;

int main(void)
{
    struct timespec deadline;
    if ((unsigned long)&primary_total != 0x402000UL)
        return 101;
    primary_total += 3;
    if (sched_yield() || primary_total != 13)
        return 102;
    if (clock_gettime(CLOCK_MONOTONIC, &deadline))
        return 103;
    deadline.tv_nsec += 100000000;
    if (deadline.tv_nsec >= 1000000000) {
        deadline.tv_nsec -= 1000000000;
        ++deadline.tv_sec;
    }
    if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, 0))
        return 104;
    struct timespec observed;
    if (clock_gettime(CLOCK_MONOTONIC, &observed) ||
        observed.tv_sec < deadline.tv_sec ||
        (observed.tv_sec == deadline.tv_sec && observed.tv_nsec < deadline.tv_nsec))
        return 105;
    if (sched_yield() || primary_total != 13)
        return 106;
    primary_total += 4;
    return (int)primary_total;
}
