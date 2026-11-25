#include "../native-edf/clock.h"

enum { blocks_per_batch = 4096 };
static volatile unsigned char ramp[256];
volatile uint64_t initial_checksum, private_result, private_blocks, private_batches;
volatile uint64_t start_ns, request_ns, wake_ns, end_ns;

__attribute__((noinline))
static unsigned square_block(void)
{
    unsigned total = 0;
    for (unsigned i = 0; i < 256; ++i) {
        unsigned x = ramp[i];
        total += x * x;
    }
    return total;
}

int main(void)
{
    uint64_t start, now;
    if (read_ns(&start) || start > UINT64_MAX - 650000000)
        return 101;
    start_ns = start;
    unsigned sum = 0;
    for (unsigned i = 0; i < 256; ++i)
        ramp[i] = (unsigned char)i;
    for (unsigned i = 0; i < 256; ++i)
        sum += ramp[i];
    initial_checksum = sum;
    if (sum != 32640)
        return 102;
    request_ns = start + 125000000;
    if (sleep_to(request_ns) || read_ns(&now) || now < request_ns)
        return 103;
    wake_ns = now;
    for (;;) {
        for (unsigned i = 0; i < blocks_per_batch; ++i) {
            unsigned delta = square_block();
            if (private_result > UINT64_MAX - delta || private_blocks == UINT64_MAX)
                return 104;
            private_result += delta;
            ++private_blocks;
        }
        ++private_batches;
        if (read_ns(&now) || now < start)
            return 105;
        if (now - start >= 650000000) {
            end_ns = now;
            return private_blocks <= UINT64_MAX / 5559680 &&
                   private_result == private_blocks * 5559680 ? 41 : 106;
        }
    }
}
