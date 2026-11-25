#include "telemetry.h"

volatile uint64_t stalled_ids[5], queue_before[4], old_handle[2], new_handle[2];
volatile unsigned char stalled_data[5][32], stalled_sizes[5];
volatile long stale_status, empty_status, wrong_publish;

static int command(unsigned expected, uint64_t *token)
{
    jnox_ipc_request r = jnox_ipc_receive();
    if (r.status || r.word != expected) return 0;
    *token = r.token;
    return 1;
}
static int consume(jnox_subscription handle, unsigned seq, unsigned row)
{
    jnox_pubsub_sample sample;
    if (jnox_receive(BROKER, handle, 0, &sample) || !sample_matches(&sample, seq)) return 0;
    stalled_ids[row] = sample.sequence;
    stalled_sizes[row] = sample.length;
    for (unsigned i = 0; i < sample.length; ++i) stalled_data[row][i] = sample.data[i];
    return 1;
}
int main(void)
{
    uint64_t token, unused;
    jnox_subscription h, next;
    jnox_pubsub_observation obs;
    jnox_pubsub_sample peek;
    const unsigned char denied = 5;
    if (!command(START, &token) || jnox_subscribe(BROKER, TOPIC, &h)) return 90;
    old_handle[0] = h.slot; old_handle[1] = h.generation;
    wrong_publish = jnox_publish(BROKER, TOPIC, &denied, 1, &unused);
    if (wrong_publish != -13 || jnox_ipc_reply(token, START)) return 91;

    if (!command(DRAIN, &token) || jnox_observe(BROKER, h, &obs)) return 92;
    queue_before[0] = obs.queued; queue_before[1] = obs.high_water;
    queue_before[2] = obs.lost; queue_before[3] = obs.front_sequence;
    if (obs.queued != 4 || obs.high_water != 4 || obs.lost != 2 || obs.front_sequence != 1) return 93;
    if (jnox_peek(BROKER, h, &peek) || !sample_matches(&peek, 1)) return 94;
    for (unsigned seq = 1; seq <= 4; ++seq)
        if (!consume(h, seq, seq-1)) return 95;
    if (jnox_receive(BROKER, h, 0, &peek) != -11 || jnox_ipc_reply(token, DRAIN)) return 96;

    if (!command(RESET, &token) || jnox_peek(BROKER, h, &peek) || !sample_matches(&peek, 7)) return 97;
    if (jnox_unsubscribe(BROKER, h) || jnox_subscribe(BROKER, TOPIC, &next)) return 98;
    new_handle[0] = next.slot; new_handle[1] = next.generation;
    stale_status = jnox_observe(BROKER, h, &obs);
    empty_status = jnox_peek(BROKER, next, &peek);
    if (stale_status != -116 || empty_status != -11 || jnox_observe(BROKER, next, &obs) ||
        obs.queued || obs.lost || jnox_ipc_reply(token, RESET)) return 99;
    if (!command(FINISH, &token) || !consume(next, 8, 4) || jnox_ipc_reply(token, FINISH)) return 100;
    // Leave the new subscription live to exercise broker cleanup on client exit.
    return 73;
}
