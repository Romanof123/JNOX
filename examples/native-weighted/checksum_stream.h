#include <stdint.h>

/* CRC-32/ISO-HDLC over repeated byte ramps. */
static volatile unsigned char block[256];
volatile uint32_t private_progress;
volatile uint32_t private_result;

__attribute__((noinline))
void checksum_stream(void)
{
    uint32_t crc = 0xffffffffU;
    for (unsigned i = 0; i < 256; ++i)
        block[i] = (unsigned char)i;
    for (unsigned n = 0; n < CRC_BLOCKS; ++n) {
        for (unsigned i = 0; i < 256; ++i) {
            crc ^= block[i];
            for (unsigned bit = 0; bit < 8; ++bit)
                crc = (crc >> 1) ^ (0xedb88320U & (0U - (crc & 1)));
        }
        private_result = ~crc;
        private_progress = (n + 1) * 256;
    }
}

int main(void)
{
    checksum_stream();
    uint32_t crc = private_result;
    return crc == CRC_EXPECTED ? (int)(crc % 251) : 101;
}
