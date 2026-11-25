#include "samples.h"

volatile uint32_t private_result;
volatile uint32_t private_progress;

__attribute__((noinline))
void urgent_reduce(void)
{
    for (unsigned block = 1; block <= 8; ++block) {
        unsigned alarms = 0;
        for (unsigned i = 0; i < BLOCK_BYTES; ++i)
            alarms += samples[i] >= 240;
        private_result += alarms;
        private_progress = block;
    }
}

int main(void)
{
    load_samples();
    urgent_reduce();
    return private_result == 2048 ? 43 : 105;
}
