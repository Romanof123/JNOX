#ifndef JNOX_PUBSUB_H
#define JNOX_PUBSUB_H
#include <stddef.h>
#include <stdint.h>

#define JNOX_PUBSUB_SAMPLE_MAX 32
typedef struct { uint64_t slot, generation; } jnox_subscription;
typedef struct {
    unsigned char queued, high_water;
    uint64_t lost, front_sequence;
} jnox_pubsub_observation;
typedef struct {
    uint64_t sequence;
    unsigned char length, data[JNOX_PUBSUB_SAMPLE_MAX];
} jnox_pubsub_sample;

/* Return zero or negative errno. Caller-owned outputs change only on success. */
long jnox_subscribe(uint64_t broker, unsigned char topic, jnox_subscription *out);
long jnox_unsubscribe(uint64_t broker, jnox_subscription sub);
long jnox_publish(uint64_t broker, unsigned char topic, const void *data,
                  size_t length, uint64_t *sequence);
long jnox_observe(uint64_t broker, jnox_subscription sub, jnox_pubsub_observation *out);
long jnox_peek(uint64_t broker, jnox_subscription sub, jnox_pubsub_sample *out);
long jnox_receive(uint64_t broker, jnox_subscription sub, int wait, jnox_pubsub_sample *out);
#endif
