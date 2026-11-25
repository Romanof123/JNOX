#include "alarms.h"
#include <sched.h>

int main(void)
{
    for (unsigned block = 0; block < 16384; ++block) {
        uint32_t count = count_alarms(block);
        if (count != 256)
            return 106;
        publish_block(count);
        if (sched_yield())
            return 107;
    }
    return private_blocks == 16384 && private_bytes == 67108864 &&
           private_result == 4194304 ? 59 : 108;
}
