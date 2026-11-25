#include "batches.h"
#include "../native-edf/clock.h"
#include <sched.h>

static volatile unsigned readings[] = {3300, 3296, 3304};
volatile uint64_t request_ns, observed_ns;

int main(int argc, char **argv)
{
    int sleep_mode = argc == 2 && equal_arg(argv[1], "sleep-400");
    if (argc != 1 && !sleep_mode)
        return 101;
    publish_batch(0, 2, readings[0] + readings[1]);
    if (sleep_mode) {
        uint64_t now;
        if (read_ns(&now) || now > UINT64_MAX - 400000000)
            return 102;
        request_ns = now + 400000000;
        if (sleep_to(request_ns) || read_ns(&now) || now < request_ns)
            return 103;
        observed_ns = now;
    } else if (sched_yield()) {
        return 104;
    }
    publish_batch(1, 1, readings[2]);
    return cumulative_sum[0] == 6596 && private_result == 9900 ? 30 : 105;
}
