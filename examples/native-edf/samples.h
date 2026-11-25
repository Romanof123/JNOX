#include <stdint.h>

enum { BLOCK_BYTES = 4096 };
static volatile unsigned char samples[BLOCK_BYTES];

static void load_samples(void)
{
    for (unsigned i = 0; i < BLOCK_BYTES; ++i)
        samples[i] = (unsigned char)(17 * i + 3);
}
