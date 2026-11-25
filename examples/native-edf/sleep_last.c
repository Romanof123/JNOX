#include "sleep_record.h"

int main(void)
{
    private_result = 31;
    if (timed_wait(140000000) || private_result != 31)
        return 113;
    /* The original absolute sleep target has already elapsed. */
    if (sleep_to(request_ns) || sched_yield())
        return 114;
    private_result += 28;
    return (int)private_result;
}
