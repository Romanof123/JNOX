#include <sched.h>
#include "boundary_common.h"

int main(void)
{
    if (grid_wait())
        return 107;
    /* A second deadline lets the other grid participant finish first. */
    uint64_t now;
    if (read_ns(&now) || sleep_to_ns(now + 40000000) || sched_yield())
        return 108;
    private_progress = 3;
    private_result = 17;
    return (int)private_result;
}
