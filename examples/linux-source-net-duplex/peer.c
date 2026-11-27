#include "protocol.h"

volatile uint64_t jnox_duplex_peer[16];
volatile long jnox_duplex_peer_tx_irq_denial;
unsigned char jnox_duplex_peer_requests[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_duplex_peer_replies[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_duplex_peer_ack[ACK_SIZE];

/* Keep a traceable call site in the peer ELF for each application reply. */
__attribute__((noinline))
void jnox_duplex_app_reply(unsigned char *out, const unsigned char *request,
                           unsigned len, unsigned seq)
{
    __builtin_memcpy(out, request + 6, 6);
    __builtin_memcpy(out + 6, request, 6);
    out[12] = 0x88;
    out[13] = 0xb6;
    __builtin_memcpy(out + 14, "JNOXRESP", 8);
    __builtin_memcpy(out + 22, request + 22, 12);
    for (unsigned i = 34; i < len; ++i)
        out[i] = request[i] ^ request[22 + ((i - 34 + seq) & 7)] ^ 0xa5;
}

static int run(void)
{
    volatile uint64_t *p = jnox_duplex_peer;
    struct duplex_session session;
    if (pipe_receive(DATA_FD, &session, sizeof(session)) || !session.token || !session.mac)
        return 40;
    p[0] = 0x4455504c50454552ULL;  /* DUPLPEER */
    p[2] = session.token;
    p[3] = session.mac;
    p[9] = native_call(164, session.token, (uintptr_t)jnox_duplex_peer_replies[0], ACK_SIZE);
    p[10] = native_call(166, session.token, 0, 0);
    p[11] = native_call(172, session.token, 0, 0);
    p[12] = native_call(174, session.token, 0, 0);
    p[14] = native_call(175, session.token, 0, 0);
    jnox_duplex_peer_tx_irq_denial = native_call(169, session.token, 2, 0);
    if (jnox_duplex_peer_tx_irq_denial != -13) return 46;
    if (p[9] != (uint64_t)-13L || p[10] != (uint64_t)-13L || p[11] != (uint64_t)-13L ||
        p[12] != (uint64_t)-13L || p[14] != (uint64_t)-13L)
        return 41;
    p[1] = 3;
    if (reply(0xa3))
        return 42;
    for (unsigned seq = 0; seq < FRAME_COUNT; ++seq) {
        unsigned len = frame_length(seq);
        unsigned char *in = jnox_duplex_peer_requests[seq];
        unsigned char *out = jnox_duplex_peer_replies[seq];
        if (pipe_receive(DATA_FD, in, len))
            return 43;
        if (!seq)
            p[4] = read_be(in + 22, 8);
        if (!valid_frame(in, len, seq, session.mac, p[4]))
            return 44;
        ++p[5];
        p[7] += len;
        jnox_duplex_app_reply(out, in, len, seq);
        if (pipe_send(REPLY_FD, out, len))
            return 45;
        ++p[6];
        p[8] += len;
    }
    p[1] = 6;
    if (pipe_receive(DATA_FD, jnox_duplex_peer_ack, ACK_SIZE) ||
        !duplex_ack(jnox_duplex_peer_ack, ACK_SIZE, session.mac, p[4]))
        return 46;
    p[13] = ACK_SIZE;
    p[1] = 100;
    return reply(0xa7) ? 47 : 0;
}

int main(void)
{
    int rc = run();
    jnox_duplex_peer[15] = rc;
    if (!rc)
        MARK("NET DUPLEX PEER PASS");
    return rc;
}
