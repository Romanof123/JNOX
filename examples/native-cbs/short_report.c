#include "ramp.h"

volatile uint32_t private_result;
volatile uint64_t private_blocks;
volatile uint64_t private_bytes;

int main(void)
{
    fill_ramp();
    unsigned peaks = 0;
    for (unsigned i = 0; i < block_bytes; ++i)
        peaks += ramp[i] >= 240;
    private_result = peaks;
    private_bytes = block_bytes;
    private_blocks = 1;
    return private_result == 256 ? 53 : 106;
}
