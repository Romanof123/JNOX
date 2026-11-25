#include <stdint.h>
#include <sched.h>
#include <jnox/periodic.h>

volatile uint32_t report_result[3];
volatile uint32_t report_bytes[3];
volatile uint32_t report_sequence[3];
volatile uint32_t report_count;

int main(void)
{
    /* Track the cumulative high-alarm count across three sample ramps. */
    volatile uint32_t cumulative = 0;
    for (unsigned g = 0; g < 3; ++g) {
        for (unsigned i = 0; i < 256; ++i)
            cumulative += (unsigned char)(17 * i + g) >= 240;
        if (sched_yield() || cumulative != 16 * (g + 1))
            return 113;
        report_result[g] = cumulative;
        report_bytes[g] = 256;
        report_sequence[g] = g + 1;
        report_count = g + 1;
        if (g < 2 && jnox_periodic_wait_next())
            return 114;
    }
    return 43;
}
