#include "ramp.h"
#include "../native-edf/clock.h"
#include <sched.h>

volatile uint32_t private_result;
volatile uint64_t private_blocks;
volatile uint64_t private_bytes;
volatile uint64_t request_ns;
volatile uint64_t observed_ns;

static void report(void)
{
    unsigned sum = 0;
    for (unsigned i = 0; i < block_bytes; ++i)
        sum += ramp[i];
    private_result += sum;
    private_bytes += block_bytes;
    ++private_blocks;
}

int main(void)
{
    uint64_t now;
    fill_ramp();
    report();
    if (read_ns(&now))
        return 107;
    request_ns = now + 125000000;
    if (sleep_to(request_ns) || read_ns(&now) || now < request_ns)
        return 108;
    observed_ns = now;
    report();
    if (sched_yield())
        return 109;
    report();
    return private_result == 1566720 ? 59 : 110;
}
