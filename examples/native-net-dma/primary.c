#include "exchange.h"

int main(void)
{
    LINE("DMA PRIMARY READY");
    int result = exchange(1);
    if (result) return result;
    LINE("DMA PRIMARY RETIRE");
    jnox_ipc_response response = jnox_ipc_call(2, RETIRE_WORD, UINT64_MAX);
    return response.status == 0 && response.word == RETIRE_WORD ? 0 : 95;
}
