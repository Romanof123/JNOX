#include <errno.h>
#include "../native-weighted/clock_helpers.h"

volatile int overflow_return;
volatile int normal_return;
volatile uint64_t request_ns;
volatile uint64_t observed_ns;
volatile uint32_t private_progress;
volatile uint32_t private_result;
static volatile unsigned char ramp[256];

__attribute__((noinline))
int request_distant_deadline(void)
{
    const struct timespec distant = {.tv_sec = 160000000000L, .tv_nsec = 0};
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &distant, 0);
}

int main(void)
{
    overflow_return = request_distant_deadline();
    if (overflow_return != EOVERFLOW)
        return 101;
    private_progress = 1;
    uint64_t now;
    if (read_ns(&now))
        return 102;
    request_ns = now + 40000000;
    normal_return = sleep_to_ns(request_ns);
    if (normal_return || read_ns(&now))
        return 103;
    observed_ns = now;
    if (now < request_ns)
        return 104;
    private_progress = 2;
    for (unsigned i = 0; i < 256; ++i)
        ramp[i] = (unsigned char)i;
    uint32_t sum = 0;
    for (unsigned i = 0; i < 256; ++i)
        sum += ramp[i];
    private_result = sum;
    private_progress = 3;
    return sum == 32640 ? 61 : 105;
}
