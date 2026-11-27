#ifndef JNOX_DUPLEX_FIXTURE_H
#define JNOX_DUPLEX_FIXTURE_H

#include "../linux-source-net-lifecycle/protocol.h"

struct duplex_session {
    uint64_t token;
    uint64_t mac;
};

enum duplex_snapshot {
    DUPLEX_PROBED, DUPLEX_OPENED, DUPLEX_HELD, DUPLEX_OVERLAP,
    DUPLEX_DELIVERED, DUPLEX_CLOSED, DUPLEX_REMOVED, DUPLEX_SNAPSHOTS
};

static inline int duplex_reply_header(const unsigned char *frame, unsigned seq,
                                       uint64_t mac, uint64_t nonce)
{
    return read_be(frame, 6) == 0x024a4e4f5801ULL && read_be(frame + 6, 6) == mac &&
           read_be(frame + 12, 2) == 0x88b6 && read_be(frame + 14, 8) == 0x4a4e4f5852455350ULL &&
           read_be(frame + 22, 8) == nonce && read_be(frame + 30, 4) == seq;
}

static inline int duplex_ack(const unsigned char *frame, unsigned len, uint64_t mac,
                             uint64_t nonce)
{
    if (!valid_ack(frame, len, mac, nonce))
        return 0;
    for (unsigned i = 30; i < len; ++i)
        if (frame[i])
            return 0;
    return 1;
}

#endif
