#include <stdint.h>
#include <jnox/periodic.h>
#include "../native-edf/clock.h"

volatile uint32_t report_result[3];
volatile uint32_t report_bytes[3];
volatile uint32_t report_sequence[3];
volatile uint32_t report_count;
volatile uint32_t active_blocks;
volatile uint64_t extra_start_ns;
volatile uint64_t extra_end_ns;
static volatile unsigned char ramp[256];

__attribute__((noinline))
uint32_t crc_ramp(uint32_t crc)
{
    for (unsigned i = 0; i < 256; ++i) {
        crc ^= ramp[i];
        for (unsigned bit = 0; bit < 8; ++bit)
            crc = (crc >> 1) ^ (0xedb88320U & (0U - (crc & 1)));
    }
    return crc;
}

static int run_crc_reports(int extend_second)
{
    const uint32_t expected[3] = {0xe81722f0U, 0xb03637e8U, 0x82e8647bU};
    unsigned sequence = 0;
    for (unsigned g = 0; g < 3; ++g) {
        int extended = extend_second && g == 1;
        uint64_t start = 0;
        if (extended && read_ns(&start))
            return 101;
        if (extended)
            extra_start_ns = start;
        for (unsigned i = 0; i < 256; ++i)
            ramp[i] = (unsigned char)(i + g);
        uint32_t crc = 0xffffffffU;
        unsigned blocks = 0;
        do {
            crc = crc_ramp(crc);
            active_blocks = ++blocks;
            if (blocks < 64 + g)
                continue;
            if (blocks == 64 + g && ~crc != expected[g])
                return 102;
            if (!extended)
                break;
            uint64_t now;
            if (read_ns(&now) || now < start)
                return 103;
            if (now - start > 110000000 && blocks > 64 + g) {
                extra_end_ns = now;
                break;
            }
            if (blocks >= 262144)
                return 104;
        } while (1);
        report_result[g] = ~crc;
        report_bytes[g] = blocks * 256;
        report_sequence[g] = ++sequence;
        report_count = sequence;
        if (g < 2 && jnox_periodic_wait_next())
            return 105;
    }
    return 17;
}
