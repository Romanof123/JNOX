#include <jnox/ipc.h>

/* Four sensor samples. The response packs count, maximum, minimum, sum. */
static const volatile unsigned short samples[] = {300, 512, 257, 1025};
volatile uint64_t ipc_request_word, ipc_reply_word;
volatile long ipc_call_status;

int main(void) {
    uint64_t request = 0;
    for (unsigned i = 0; i < 4; ++i)
        request |= (uint64_t)samples[i] << (16 * i);
    ipc_request_word = request;
    jnox_ipc_response reply = jnox_ipc_call(1, request, UINT64_MAX);
    ipc_call_status = reply.status;
    if (reply.status != 0) return 81;
    ipc_reply_word = reply.word;
    return reply.word == 0x000404010101082eULL ? 41 : 82;
}
