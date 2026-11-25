#include "stream.h"

volatile uint32_t private_result;
static uint32_t expected_crc;

/* CRC concatenation for this fixed ramp is affine in the previous CRC.
 * Column i = zlib.crc32(ramp, 1 << i) XOR zlib.crc32(ramp, 0). */
static const uint32_t columns[32] = {
    0x5ad8a92cU, 0xb5b15258U, 0xb013a2f1U, 0xbb5643a3U,
    0xaddd8107U, 0x80ca044fU, 0xdae50edfU, 0x6ebb1bffU,
    0xdd7637feU, 0x619d69bdU, 0xc33ad37aU, 0x5d04a0b5U,
    0xba09416aU, 0xaf638495U, 0x85b60f6bU, 0xd01d1897U,
    0x7b4b376fU, 0xf6966edeU, 0x365ddbfdU, 0x6cbbb7faU,
    0xd9776ff4U, 0x699fd9a9U, 0xd33fb352U, 0x7d0e60e5U,
    0xfa1cc1caU, 0x2f4885d5U, 0x5e910baaU, 0xbd221754U,
    0xa13528e9U, 0x991b5793U, 0xe947a967U, 0x09fe548fU,
};

__attribute__((noinline))
static void reduce_block(void)
{
    uint32_t crc = ~private_result;
    for (unsigned i = 0; i < block_bytes; ++i) {
        crc ^= ramp[i];
        for (unsigned bit = 0; bit < 8; ++bit)
            crc = (crc >> 1) ^ (0xedb88320U & (0U - (crc & 1)));
    }
    private_result = ~crc;
    uint32_t previous = expected_crc;
    expected_crc = 0xa2912082U;
    for (unsigned i = 0; previous != 0; ++i, previous >>= 1)
        if (previous & 1U)
            expected_crc ^= columns[i];
}

static int checked_status(void)
{
    return private_result == expected_crc && private_bytes == private_blocks * block_bytes ? 17 : 104;
}

int main(void)
{
    return run_stream();
}
