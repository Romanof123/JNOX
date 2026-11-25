#include "telemetry.h"

volatile uint64_t published_ids[8];
volatile unsigned char published_bytes[8][32], published_sizes[8];
volatile long publisher_subscribe_status;

static int ask(unsigned slot, unsigned command)
{
    jnox_ipc_response response = jnox_ipc_call(slot, command, UINT64_MAX);
    return response.status == 0 && response.word == command;
}

int main(void)
{
    jnox_subscription denied;
    publisher_subscribe_status = jnox_subscribe(BROKER, TOPIC, &denied);
    if (publisher_subscribe_status != -13) return 90;
    if (!ask(1, START) || !ask(2, START)) return 91;
    for (unsigned seq = 1; seq <= 8; ++seq) {
        unsigned char payload[32];
        unsigned len = lengths[seq-1];
        for (unsigned i = 0; i < len; ++i) {
            payload[i] = telemetry_byte(seq, i);
            published_bytes[seq-1][i] = payload[i];
        }
        uint64_t id;
        if (jnox_publish(BROKER, TOPIC, payload, len, &id) || id != seq) return 92;
        published_ids[seq-1] = id;
        published_sizes[seq-1] = (unsigned char)len;
        if (!ask(1, seq)) return 93;
        if (seq == 6 && !ask(2, DRAIN)) return 94;
        if (seq == 7 && !ask(2, RESET)) return 95;
    }
    if (!ask(2, FINISH) || !ask(1, FINISH)) return 96;
    return 71;
}
