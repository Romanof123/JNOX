#include <stdint.h>

enum { block_bytes = 4096 };
static volatile unsigned char samples[block_bytes];
volatile uint32_t private_blocks;
volatile uint32_t private_result;
volatile uint64_t private_bytes;

__attribute__((noinline))
uint32_t count_alarms(unsigned round)
{
    for (unsigned i = 0; i < block_bytes; ++i)
        samples[i] = (unsigned char)(17 * i + 3 + round);
    uint32_t count = 0;
    for (unsigned i = 0; i < block_bytes; ++i)
        count += samples[i] >= 240;
    return count;
}

static void publish_block(uint32_t count)
{
    private_result += count;
    private_bytes += block_bytes;
    ++private_blocks;
}
