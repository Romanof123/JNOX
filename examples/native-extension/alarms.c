#include "batches.h"
#include <sched.h>

static volatile unsigned char ramp[256];

__attribute__((noinline))
unsigned count_alarms(void)
{
    unsigned total = 0;
    for (unsigned block = 0; block < 4096; ++block)
        for (unsigned i = 0; i < 256; ++i)
            total += ramp[i] >= 240;
    return total;
}

int main(int argc, char **argv)
{
    unsigned batches = 3;
    if (argc == 2 && equal_arg(argv[1], "four-batches"))
        batches = 4;
    else if (argc != 1)
        return 106;
    for (unsigned i = 0; i < 256; ++i)
        ramp[i] = (unsigned char)i;
    for (unsigned batch = 0; batch < batches; ++batch) {
        unsigned hits = count_alarms();
        publish_batch(batch, 4096 * 256, hits);
        if (hits != 65536 || private_result != (batch + 1) * 65536)
            return 107;
        if (batch + 1 < batches && sched_yield())
            return 108;
    }
    return 32;
}
