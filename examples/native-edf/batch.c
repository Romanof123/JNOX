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

static uint32_t parse_blocks(const char *option, const char *text)
{
    const char *expected = "--blocks";
    while (*expected && *option == *expected) {
        ++option;
        ++expected;
    }
    if (*expected || *option || !*text)
        return 0;
    uint32_t count = 0;
    for (; *text; ++text) {
        if (*text < '0' || *text > '9')
            return 0;
        count = count * 10 + (uint32_t)(*text - '0');
        if (count > 65536)
            return 0;
    }
    return count;
}

static int timed_batch(void)
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

int main(int argc, char **argv)
{
    if (argc == 1)
        return timed_batch();
    uint32_t count = argc == 3 ? parse_blocks(argv[1], argv[2]) : 0;
    if (!count)
        return 105;
    load_samples();
    for (uint32_t n = 1; n <= count; ++n) {
        private_result += batch_reduce();
        private_progress = n;
    }
    return private_result == 522240ULL * count ? 17 : 103;
}
