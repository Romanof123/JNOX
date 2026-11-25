#include <errno.h>
#include <jnox/ipc.h>
#include <jnox/pubsub.h>

enum { SUBSCRIBE = 1, UNSUBSCRIBE, PUBLISH, OBSERVE, RECEIVE, PEEK };
enum { MAX_RESPONSE = 9 + JNOX_PUBSUB_SAMPLE_MAX };
static unsigned char active;

static long rpc(uint64_t broker, uint64_t frame) {
    jnox_ipc_response r = jnox_ipc_call(broker, frame, UINT64_MAX);
    return r.status ? r.status : (long)r.word;
}

static uint64_t le64(const unsigned char *p) {
    uint64_t value = 0;
    for (unsigned i = 0; i < 8; ++i) value |= (uint64_t)p[i] << (i * 8);
    return value;
}
static void put64(unsigned char *p, uint64_t value) {
    for (unsigned i = 0; i < 8; ++i) p[i] = (unsigned char)(value >> (i * 8));
}

/* The lock covers all frames, including a blocked COMMIT and its readback. */
static long request(uint64_t broker, unsigned op, unsigned topic, unsigned wait,
                    const unsigned char *in, unsigned len, unsigned char *out,
                    unsigned minimum, unsigned maximum) {
    if (__atomic_test_and_set(&active, __ATOMIC_ACQUIRE)) return -EBUSY;
    uint64_t begin = (0x31ULL << 56) | ((uint64_t)wait << 24) | ((uint64_t)op << 16)
                   | ((uint64_t)topic << 8) | len;
    long result = rpc(broker, begin);
    if (result != 0) {
        if (result > 0) result = -EIO;
        goto done;
    }
    for (unsigned off = 0; off < len; off += 6) {
        uint64_t chunk = (0x32ULL << 56) | ((uint64_t)off << 48);
        for (unsigned j = 0; j < 6 && off + j < len; ++j)
            chunk |= (uint64_t)in[off + j] << (8 * j);
        result = rpc(broker, chunk);
        if (result != 0) {
            if (result > 0) result = -EIO;
            (void)rpc(broker, 0x35ULL << 56);
            goto done;
        }
    }
    result = rpc(broker, 0x33ULL << 56);
    if (result < 0) goto done;
    if ((unsigned long)result > maximum || (unsigned long)result < minimum) {
        result = -EIO;
        goto done;
    }
    for (unsigned off = 0; off < (unsigned)result; off += 6) {
        long word = rpc(broker, (0x34ULL << 56) | ((uint64_t)off << 48));
        if (word < 0) { result = word; goto done; }
        unsigned n = (unsigned)result - off;
        if (n > 6) n = 6;
        if ((uint64_t)word >> (n * 8)) { result = -EIO; goto done; }
        for (unsigned j = 0; j < n; ++j)
            out[off + j] = (unsigned char)((uint64_t)word >> (8 * j));
    }
done:
    __atomic_clear(&active, __ATOMIC_RELEASE);
    return result;
}

static long subscription_request(uint64_t broker, unsigned op, unsigned wait,
                                 jnox_subscription sub, unsigned char *out,
                                 unsigned minimum, unsigned maximum) {
    unsigned char in[16];
    put64(in, sub.slot);
    put64(in + 8, sub.generation);
    return request(broker, op, 0, wait, in, sizeof in, out, minimum, maximum);
}

long jnox_subscribe(uint64_t broker, unsigned char topic, jnox_subscription *out) {
    if (!out) return -EFAULT;
    unsigned char raw[16];
    long r = request(broker, SUBSCRIBE, topic, 0, 0, 0, raw, sizeof raw, sizeof raw);
    if (r < 0) return r;
    uint64_t generation = le64(raw + 8);
    if (!generation) return -EIO;
    *out = (jnox_subscription){le64(raw), generation};
    return 0;
}
long jnox_unsubscribe(uint64_t broker, jnox_subscription sub) {
    return subscription_request(broker, UNSUBSCRIBE, 0, sub, 0, 0, 0);
}
long jnox_publish(uint64_t broker, unsigned char topic, const void *data,
                  size_t length, uint64_t *sequence) {
    if (length > JNOX_PUBSUB_SAMPLE_MAX) return -EMSGSIZE;
    if (!sequence || (length && !data)) return -EFAULT;
    unsigned char raw[8];
    long r = request(broker, PUBLISH, topic, 0, data, (unsigned)length, raw, sizeof raw, sizeof raw);
    if (r < 0) return r;
    *sequence = le64(raw);
    return 0;
}
long jnox_observe(uint64_t broker, jnox_subscription sub, jnox_pubsub_observation *out) {
    if (!out) return -EFAULT;
    unsigned char raw[18];
    long r = subscription_request(broker, OBSERVE, 0, sub, raw, sizeof raw, sizeof raw);
    if (r < 0) return r;
    uint64_t front = le64(raw + 10);
    if (raw[0] > raw[1] || (!raw[0] && front)) return -EIO;
    *out = (jnox_pubsub_observation){raw[0], raw[1], le64(raw + 2), front};
    return 0;
}

static long sample_request(uint64_t broker, unsigned op, unsigned wait,
                           jnox_subscription sub, jnox_pubsub_sample *out) {
    if (!out) return -EFAULT;
    unsigned char raw[MAX_RESPONSE];
    long r = subscription_request(broker, op, wait, sub, raw, 9, sizeof raw);
    if (r < 0) return r;
    if (r != 9 + raw[8]) return -EIO;
    out->sequence = le64(raw);
    out->length = raw[8];
    for (unsigned i = 0; i < JNOX_PUBSUB_SAMPLE_MAX; ++i)
        out->data[i] = i < raw[8] ? raw[9 + i] : 0;
    return 0;
}
long jnox_peek(uint64_t broker, jnox_subscription sub, jnox_pubsub_sample *out) {
    return sample_request(broker, PEEK, 0, sub, out);
}
long jnox_receive(uint64_t broker, jnox_subscription sub, int wait, jnox_pubsub_sample *out) {
    return sample_request(broker, RECEIVE, !!wait, sub, out);
}
