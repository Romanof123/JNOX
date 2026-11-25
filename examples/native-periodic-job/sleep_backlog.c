#include "../native-edf/clock.h"

volatile uint64_t request_ns;
volatile uint64_t observed_ns;
volatile uint32_t report_count;

int main(void)
{
    uint64_t now;
    if (read_ns(&now))
        return 115;
    request_ns = now + 65000000;
    if (sleep_to(request_ns) || read_ns(&now) || now < request_ns)
        return 116;
    observed_ns = now;
    report_count = 1;
    return 53;
}
