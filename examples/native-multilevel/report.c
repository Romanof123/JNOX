#include "alarms.h"
#include "../native-edf/clock.h"

volatile uint64_t request_ns[3];
volatile uint64_t observed_ns[3];
volatile uint32_t round_count[3];
volatile uint32_t cumulative_count[3];

int main(void)
{
    for (unsigned round = 0; round < 3; ++round) {
        uint64_t now;
        if (read_ns(&now))
            return 101;
        request_ns[round] = now + 40000000;
        if (sleep_to(request_ns[round]) || read_ns(&now))
            return 102;
        observed_ns[round] = now;
        if (now < request_ns[round])
            return 103;
        uint32_t count = count_alarms(round);
        publish_block(count);
        round_count[round] = count;
        cumulative_count[round] = private_result;
        if (count != 256 || private_result != (round + 1) * 256)
            return 104;
    }
    return private_bytes == 12288 && private_blocks == 3 ? 53 : 105;
}
