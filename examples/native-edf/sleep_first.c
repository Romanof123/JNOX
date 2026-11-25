#include "sleep_record.h"

int main(void)
{
    private_result = 23;
    if (sched_yield() || timed_wait(40000000) || private_result != 23)
        return 112;
    private_result += 30;
    return (int)private_result;
}
