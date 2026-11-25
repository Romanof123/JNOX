#include <stdint.h>

volatile uint32_t private_batches, batch_count[4];
volatile uint64_t private_result, batch_sum[4], cumulative_sum[4];

static void publish_batch(unsigned batch, unsigned count, uint64_t sum)
{
    batch_count[batch] = count;
    batch_sum[batch] = sum;
    private_result += sum;
    cumulative_sum[batch] = private_result;
    private_batches = batch + 1;
}

static int equal_arg(const char *arg, const char *expected)
{
    while (*arg && *arg == *expected) {
        ++arg;
        ++expected;
    }
    return *arg == *expected;
}
