#include "sleep_record.h"

int main(void)
{
    private_result = 41;
    if (timed_wait(40000000) || private_result != 41)
        return 115;
    private_result += 20;
    return (int)private_result;
}
