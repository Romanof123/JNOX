#include <stdint.h>

/* The prime 2 is already counted. Both workers publish each new prime count
 * in their own first writable page so a debugger can inspect their progress. */
__attribute__((section(".data.private_report")))
volatile uint32_t private_result = 1;

__attribute__((noinline))
void count_low_primes(void)
{
    for (uint32_t candidate = 3; candidate < 1000000; candidate += 2) {
        uint32_t divisor = 3;
        while (divisor * divisor <= candidate && candidate % divisor != 0)
            divisor += 2;
        if (divisor * divisor > candidate)
            ++private_result;
    }
}

static int low_prime_status(void)
{
    uint32_t count = private_result;
    return count == 78498 ? (int)(count % 127) : 101;
}
