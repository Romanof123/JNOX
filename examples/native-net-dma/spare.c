#include "exchange.h"

volatile long dma_spare_preclaim, dma_spare_claim;
int main(void)
{
    dma_spare_preclaim = claim();
    if (dma_spare_preclaim != -11) return 96;
    jnox_ipc_request req = jnox_ipc_receive();
    if (req.status || req.word != RETIRE_WORD) return 97;
    if (jnox_ipc_reply(req.token, RETIRE_WORD)) return 98;
    LINE("DMA SPARE WAIT");
    if (pause_ms(15000)) return 99;
    dma_spare_claim = claim();
    if (dma_spare_claim) return 100;
    LINE("DMA SPARE READY");
    int result = exchange(2);
    if (result) return result;
    LINE("DMA SPARE RETIRE");
    return jnox_event_set(1, 1) == 0 ? 0 : 103;
}
