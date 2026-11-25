#include <stdint.h>

static volatile unsigned char ramp[4096];
volatile uint64_t private_result;
volatile uint32_t private_blocks;

__attribute__((noinline))
void checksum_reduce(void)
{
    for (unsigned n = 1; n <= 16; ++n) {
        unsigned sum = 0;
        for (unsigned i = 0; i < sizeof ramp; ++i)
            sum += ramp[i];
        private_result += sum;
        private_blocks = n;
    }
}

int main(void)
{
    for (unsigned i = 0; i < sizeof ramp; ++i)
        ramp[i] = (unsigned char)(17 * i + 3);
    checksum_reduce();
    return private_blocks == 16 && private_result == 8355840 ? 17 : 101;
}
