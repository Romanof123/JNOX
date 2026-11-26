#include "common.h"

volatile uint64_t dma_echo_count;
volatile uint64_t dma_ignored;
static int exchange(unsigned epoch)
{
    static const unsigned char host[6] = {2,0,0,0,0,1};
    static const unsigned char guest[6] = {0x52,0x54,0,0x12,0x34,0x56};
    for (;;) {
        long n = raw(RX, sizeof(frame));
        if (n == -11) {
            if (pause_ms(5)) return 91;
            continue;
        }
        if (n < 14 || n > (long)sizeof(frame)) return 92;
        if (n != 60 || frame[12] != 0x88 || frame[13] != 0xb5 ||
            frame[14] != 'J' || frame[15] != 'N' || frame[16] != 'D' ||
            frame[17] != 'M' || frame[18] != epoch) {
            ++dma_ignored;
            continue;
        }
        if (frame[19] == 3 && dma_echo_count) return 0;
        if (frame[19] != 1) continue;
        for (unsigned i = 0; i < 6; ++i) {
            frame[i] = host[i];
            frame[6+i] = guest[i];
        }
        frame[19] = 2;
        do {
            n = raw(TX, 60);
            if (n == -11 && pause_ms(5)) return 93;
        } while (n == -11);
        if (n != 60) return 94;
        ++dma_echo_count;
    }
}
