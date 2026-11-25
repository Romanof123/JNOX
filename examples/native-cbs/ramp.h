#include <stdint.h>

enum { block_bytes = 4096 };
static volatile unsigned char ramp[block_bytes];

static void fill_ramp(void)
{
    for (unsigned i = 0; i < block_bytes; ++i)
        ramp[i] = (unsigned char)i;
}
