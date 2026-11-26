#include "exchange.h"

volatile long dma_spare_preclaim, dma_spare_claim, dma_spare_duplicate;
int main(void)
{
    dma_spare_preclaim = jnox_ethernet_claim();
    if (dma_spare_preclaim != -11) return 96;
    jnox_ipc_request req = jnox_ipc_receive();
    if (req.status || req.word != RETIRE_WORD) return 97;
    if (jnox_ipc_reply(req.token, RETIRE_WORD)) return 98;
    dma_phase = 1;
    if (pause_ms(15000)) return 99;
    dma_spare_claim = jnox_ethernet_claim();
    if (dma_spare_claim) return 100;
    dma_spare_duplicate = jnox_ethernet_claim();
    if (dma_spare_duplicate != -114) return 106;
    dma_phase = 2;
    int result = exchange(2);
    if (result) return result;
    dma_phase = 3;
    return jnox_event_set(1, 1) == 0 ? 0 : 103;
}
