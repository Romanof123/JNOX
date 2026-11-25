#include "clock.h"
#include <sched.h>

volatile uint32_t private_progress;
volatile uint32_t private_result;
volatile uint64_t request_ns;
volatile uint64_t observed_ns;

static int timed_wait(uint64_t delay)
{
    uint64_t now;
    if (read_ns(&now))
        return -1;
    request_ns = now + delay;
    private_progress = 1;
    if (sleep_to(request_ns) || read_ns(&now))
        return -1;
    observed_ns = now;
    private_progress = 2;
    return now >= request_ns ? 0 : -1;
}
