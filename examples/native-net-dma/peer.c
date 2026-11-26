#include "common.h"

volatile uint64_t dma_peer_ticks, dma_peer_ns;
volatile uint64_t dma_peer_finished;
volatile long dma_peer_rx, dma_peer_tx, dma_peer_claim;
int main(void)
{
    for (;;) {
        dma_peer_rx = raw(RX, sizeof(frame));
        dma_peer_tx = raw(TX, 60);
        dma_peer_claim = claim();
        if (dma_peer_rx != -13 || dma_peer_tx != -13 || dma_peer_claim != -13)
            return 101;
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now)) return 104;
        dma_peer_ns = (uint64_t)now.tv_sec * 1000000000ULL + now.tv_nsec;
        ++dma_peer_ticks;
        LINE("DMA PEER TICK");
        if (dma_peer_finished && dma_peer_ticks >= dma_peer_finished + 5) return 0;
        jnox_event_result done = jnox_event_take(1, 0);
        if (done.status == 0 && done.bits == 1) dma_peer_finished = dma_peer_ticks;
        else if (done.status != -11) return 105;
        if (pause_ms(1000)) return 102;
    }
}
