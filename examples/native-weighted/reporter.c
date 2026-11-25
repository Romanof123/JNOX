#include "clock_helpers.h"

/* Three observations, each taken after a fresh 40 ms monitoring interval. */
volatile uint64_t report_deadline_ns[3];
volatile uint64_t report_observed_ns[3];
volatile uint32_t private_progress;
volatile uint32_t private_result;

int main(void)
{
    for (unsigned i = 0; i < 3; ++i) {
        uint64_t now;
        if (read_ns(&now))
            return 104;
        uint64_t deadline = now + 40000000;
        report_deadline_ns[i] = deadline;
        if (sleep_to_ns(deadline) || read_ns(&now))
            return 105;
        report_observed_ns[i] = now;
        if (now < deadline)
            return 106;
        private_result |= 1U << i;
        private_progress = i + 1;
    }
    return (int)private_result;
}
