#include "stream.h"

volatile uint64_t private_result;

__attribute__((noinline))
static void reduce_block(void)
{
    uint64_t energy = private_result;
    for (unsigned i = 0; i < block_bytes; ++i) {
        int centered = (int)ramp[i] - 128;
        energy += (unsigned)(centered * centered);
    }
    private_result = energy;
}

static int checked_status(void)
{
    return private_result == private_blocks * 22370304ULL ? 43 : 105;
}

int main(void)
{
    return run_stream();
}
