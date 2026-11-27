#include "../native-edf/clock.h"

#define PERIOD_NS 10000000ULL
#define CAPACITY 4096
volatile uint64_t restart_control_count;
volatile uint64_t restart_control_deadline[CAPACITY];
volatile uint64_t restart_control_wake[CAPACITY];
volatile uint64_t restart_control_max_gap;
volatile uint64_t restart_control_missed;

int main(void)
{
    uint64_t deadline, previous;
    if (read_ns(&deadline)) return 110;
    previous = deadline;
    for (unsigned i = 0; i < CAPACITY; ++i) {
        uint64_t wake;
        deadline += PERIOD_NS;
        if (sleep_to(deadline) || read_ns(&wake) || wake < deadline) return 111;
        restart_control_deadline[i] = deadline;
        restart_control_wake[i] = wake;
        if (wake - previous > restart_control_max_gap) restart_control_max_gap = wake - previous;
        if (wake >= deadline + PERIOD_NS) ++restart_control_missed;
        previous = wake;
        restart_control_count = i + 1;
    }
    return 74;
}
