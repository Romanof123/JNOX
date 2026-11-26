#include "exchange.h"

int main(void)
{
    dma_phase = 1;
    int result = exchange(1);
    if (result) return result;
    dma_phase = 2;
    jnox_ipc_response response = jnox_ipc_call(2, RETIRE_WORD, UINT64_MAX);
    return response.status == 0 && response.word == RETIRE_WORD ? 0 : 95;
}
