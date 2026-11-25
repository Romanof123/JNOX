#include "samples.h"
#include "clock.h"

volatile uint64_t private_result;
volatile uint32_t private_progress;

__attribute__((noinline))
uint32_t batch_reduce(void)
{
    uint32_t sum = 0;
    for (unsigned i = 0; i < BLOCK_BYTES; ++i)
        sum += samples[i];
    return sum;
}

int main(void)
{
    uint64_t start, now;
    if (read_ns(&start))
        return 101;
    load_samples();
    for (uint32_t n = 1; n <= 65536; ++n) {
        private_result += batch_reduce();
        private_progress = n;
        if (read_ns(&now) || now < start)
            return 102;
        if (now - start >= 160000000)
            return private_result == 522240ULL * n ? 17 : 103;
    }
    return 104;
}
