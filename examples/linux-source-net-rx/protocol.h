#ifndef JNOX_RX_FIXTURE_PROTOCOL_H
#define JNOX_RX_FIXTURE_PROTOCOL_H

#include <stdint.h>

enum { DATA_FD = 3, REPLY_FD = 4, FRAME_COUNT = 80, MAX_FRAME = 91, ACK_SIZE = 60 };
/* Bit 63 in the IPC hello's MAC word selects the extra peer callback check. */
#define RX_POLL_SESSION (1ULL << 63)

static inline long native_call(unsigned long service, unsigned long a, unsigned long b, unsigned long c)
{
    register unsigned long x0 __asm__("x0") = a;
    register unsigned long x1 __asm__("x1") = b;
    register unsigned long x2 __asm__("x2") = c;
    register unsigned long x8 __asm__("x8") = service;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2) : "r"(x8) : "cc", "memory");
    return (long)x0;
}

/* Each record fits in PIPE_BUF. A native pipe read can return a prefix. */
static inline int pipe_send(unsigned fd, const void *bytes, unsigned len)
{
    return native_call(47, fd, (uintptr_t)bytes, len) == (long)len ? 0 : -1;
}

static inline int pipe_receive(unsigned fd, void *out, unsigned len)
{
    unsigned char *next = out;
    while (len) {
        long n = native_call(46, fd, (uintptr_t)next, len);
        if (n <= 0 || (unsigned long)n > len) return -1;
        next += n;
        len -= (unsigned)n;
    }
    return 0;
}

static inline uint64_t read_be(const unsigned char *data, unsigned len)
{
    uint64_t value = 0;
    for (unsigned i = 0; i < len; ++i) value = (value << 8) | data[i];
    return value;
}

static inline unsigned frame_length(unsigned seq)
{
    return 60 + (seq % 32);
}

static inline int matching_header(const unsigned char *frame, uint64_t mac, const char magic[8])
{
    if (read_be(frame, 6) != mac || read_be(frame + 6, 6) != 0x024a4e4f5801ULL ||
        frame[12] != 0x88 || frame[13] != 0xb5) return 0;
    for (unsigned i = 0; i < 8; ++i) {
        if (frame[14 + i] != (unsigned char)magic[i]) return 0;
    }
    return 1;
}

static inline int valid_frame(const unsigned char *frame, unsigned len, unsigned seq,
                             uint64_t mac, uint64_t nonce)
{
    if (len != frame_length(seq) || !matching_header(frame, mac, "JNOXRECV") ||
        read_be(frame + 22, 8) != nonce || read_be(frame + 30, 4) != seq) return 0;
    for (unsigned i = 34; i < len; ++i) {
        if (frame[i] != (unsigned char)(seq + i)) return 0;
    }
    return 1;
}

static inline int valid_ack(const unsigned char *frame, unsigned len, uint64_t mac, uint64_t nonce)
{
    return len == ACK_SIZE && matching_header(frame, mac, "JNOXACK!") && read_be(frame + 22, 8) == nonce;
}

#endif
