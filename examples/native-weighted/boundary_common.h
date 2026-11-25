#include "clock_helpers.h"

volatile uint32_t private_progress;
volatile uint32_t private_result;
volatile uint64_t boundary_deadline_ns;

/* Aim both clients at the same 200 ms reporting grid, giving the peer at
 * least 40 ms to enter. The trace determines whether both targets matched. */
static int grid_wait(void)
{
    uint64_t now;
    if (read_ns(&now))
        return -1;
    uint64_t target = ((now + 40000000 + 199999999) / 200000000) * 200000000;
    boundary_deadline_ns = target;
    private_progress = 1;
    if (sleep_to_ns(target) || read_ns(&now) || now < target)
        return -1;
    private_progress = 2;
    return 0;
}
