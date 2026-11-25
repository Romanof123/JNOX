#include <sched.h>
#include "low_prime_work.h"

int main(void)
{
    count_low_primes();
    if (sched_yield())
        return 102;
    return low_prime_status();
}
