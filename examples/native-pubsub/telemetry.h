#ifndef TELEMETRY_H
#define TELEMETRY_H
#include <stdint.h>
#include <jnox/ipc.h>
#include <jnox/pubsub.h>

enum { BROKER = 3, TOPIC = 0, START = 100, DRAIN = 200, RESET = 201, FINISH = 202 };
static const unsigned char lengths[] = { 0, 1, 7, 32, 11, 3, 19, 6 };

static unsigned char telemetry_byte(unsigned sequence, unsigned index)
{
    return (unsigned char)(sequence * 43 + index * 19 + (index >> 1));
}

static int sample_matches(const jnox_pubsub_sample *s, unsigned sequence)
{
    if (s->sequence != sequence || s->length != lengths[sequence-1]) return 0;
    for (unsigned i = 0; i < s->length; ++i)
        if (s->data[i] != telemetry_byte(sequence, i)) return 0;
    return 1;
}
#endif
