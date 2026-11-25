#include "../native-multilevel/alarms.h"
#include "../native-edf/clock.h"
#include <sched.h>

volatile uint32_t round_count[3], cumulative_count[3], round_stage[3];
volatile uint64_t request_ns[3], observed_ns[3];

int main(void)
{
    for (unsigned r = 0; r < 3; ++r) {
        uint32_t count = count_alarms(r);
        publish_block(count);
        round_count[r] = count;
        cumulative_count[r] = private_result;
        if (count != 256 || private_result != (r + 1) * 256)
            return 101;
        round_stage[r] = 1;
        if (sched_yield())
            return 102;
        round_stage[r] = 2;
        uint64_t now;
        if (read_ns(&now) || now > UINT64_MAX - 40000000)
            return 103;
        request_ns[r] = now + 40000000;
        if (sleep_to(request_ns[r]) || read_ns(&now))
            return 104;
        observed_ns[r] = now;
        if (now < request_ns[r])
            return 105;
        round_stage[r] = 3;
    }
    return private_blocks == 3 && private_bytes == 12288 && private_result == 768 ? 53 : 106;
}
