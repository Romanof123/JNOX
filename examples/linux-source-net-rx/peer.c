#include "protocol.h"
#include <unistd.h>

volatile uint64_t jnox_rx_peer_result[32];
unsigned char jnox_rx_peer_frames[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_rx_peer_ack[ACK_SIZE];
#define R jnox_rx_peer_result
static unsigned char ungranted_buffer[1530];

static int consume(void)
{
    uint64_t session[2];
    if (pipe_receive(DATA_FD, session, sizeof(session)) || !session[0]) return 40;
    R[10] = session[1] & ~RX_POLL_SESSION;
    R[3] = (uint64_t)native_call(170, session[0], 0, 0);
    R[4] = (uint64_t)native_call(171, session[0], (uintptr_t)ungranted_buffer, sizeof(ungranted_buffer));
    R[5] = (uint64_t)native_call(172, session[0], 0, 0);
    if (R[3] != (uint64_t)-13L || R[4] != (uint64_t)-13L || R[5] != (uint64_t)-13L) return 41;
    if (session[1] & RX_POLL_SESSION) {
        R[12] = (uint64_t)native_call(173, session[0], 2, 0);
        if (R[12] != (uint64_t)-13L) return 48;
    }
    unsigned char reply = 0xa3;
    R[1] = 3;
    if (pipe_send(REPLY_FD, &reply, 1)) return 42;
    for (unsigned seq = 0; seq < FRAME_COUNT; ++seq) {
        unsigned char *received = jnox_rx_peer_frames[seq];
        unsigned len = frame_length(seq);
        if (pipe_receive(DATA_FD, received, len)) return 43;
        uint64_t nonce = seq ? R[8] : read_be(received + 22, 8);
        if (!valid_frame(received, len, seq, R[10], nonce)) return 44;
        if (!seq) {
            R[8] = nonce;
            R[1] = 4;
        }
        ++R[6];
        R[7] += len;
        R[9] = seq;
    }
    R[1] = 6;
    reply = 0xa6;
    if (pipe_send(REPLY_FD, &reply, 1)) return 45;
    if (pipe_receive(DATA_FD, jnox_rx_peer_ack, ACK_SIZE) ||
        !valid_ack(jnox_rx_peer_ack, ACK_SIZE, R[10], R[8])) return 46;
    R[11] = ACK_SIZE;
    reply = 0xa7;
    if (pipe_send(REPLY_FD, &reply, 1)) return 47;
    return 0;
}

int main(void)
{
    R[0] = 0x4a4e525850454531ULL;
    R[1] = 1;
    int rc = consume();
    R[2] = (unsigned)rc;
    if (rc) return rc;
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET RX PEER PASS\n", sizeof("NET RX PEER PASS\n") - 1);
    return 0;
}
