#include "ramp.h"
#include "../native-edf/clock.h"

volatile uint64_t private_blocks;
volatile uint64_t private_bytes;

static void reduce_block(void);
static int checked_status(void);

static int run_stream(void)
{
    uint64_t start;
    if (read_ns(&start))
        return 101;
    fill_ramp();
    for (unsigned n = 1; n <= (1U << 20); ++n) {
        reduce_block();
        private_bytes = (uint64_t)n * block_bytes;
        private_blocks = n;
        uint64_t now;
        if (read_ns(&now) || now < start)
            return 102;
        if (now - start >= 650000000)
            return checked_status();
    }
    return 103;
}
