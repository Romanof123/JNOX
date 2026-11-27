#include "protocol.h"
#include <unistd.h>
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
#include <time.h>
volatile uint64_t jnox_rx_irq_peer[16];
#define IRQ_PEER jnox_rx_irq_peer

static int quiet_ticks(void)
{
    unsigned char go;
    if (pipe_receive(DATA_FD, &go, 1) || go != 0xb1) return 51;
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return 52;
    uint64_t base = (uint64_t)t.tv_sec * 1000000000ULL + t.tv_nsec;
    IRQ_PEER[4] = base;
    for (unsigned i = 1; i <= 3; ++i) {
        uint64_t deadline = base + i * 10000000ULL;
        struct timespec target = {(time_t)(deadline / 1000000000ULL), (long)(deadline % 1000000000ULL)};
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0) ||
            clock_gettime(CLOCK_MONOTONIC, &t)) return 53;
        uint64_t observed = (uint64_t)t.tv_sec * 1000000000ULL + t.tv_nsec;
        if (observed < deadline || observed <= IRQ_PEER[3 + i]) return 54;
        IRQ_PEER[4 + i] = observed;
        IRQ_PEER[8] = i;
    }
    (void)write(STDOUT_FILENO, "NET IRQ QUIET PEER\n", sizeof("NET IRQ QUIET PEER\n") - 1);
    return 0;
}

static int first_frame_tick(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now)) return 64;
    IRQ_PEER[9] = (uint64_t)now.tv_sec * 1000000000ULL + now.tv_nsec;
    IRQ_PEER[10] = IRQ_PEER[9] + 10000000ULL;
    struct timespec target = {(time_t)(IRQ_PEER[10] / 1000000000ULL),
                              (long)(IRQ_PEER[10] % 1000000000ULL)};
    if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0) ||
        clock_gettime(CLOCK_MONOTONIC, &now)) return 65;
    IRQ_PEER[11] = (uint64_t)now.tv_sec * 1000000000ULL + now.tv_nsec;
    return IRQ_PEER[11] >= IRQ_PEER[10] ? 0 : 66;
}
#endif

volatile uint64_t jnox_rx_peer_result[32];
unsigned char jnox_rx_peer_frames[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_rx_peer_ack[ACK_SIZE];
#define R jnox_rx_peer_result
static unsigned char ungranted_buffer[1530];

static int consume(void)
{
    uint64_t session[2];
    if (pipe_receive(DATA_FD, session, sizeof(session)) || !session[0]) return 40;
    R[10] = session[1] & ~(RX_POLL_SESSION | RX_IRQ_SESSION | RX_IRQ_RACE_SESSION);
    R[3] = (uint64_t)native_call(170, session[0], 0, 0);
    R[4] = (uint64_t)native_call(171, session[0], (uintptr_t)ungranted_buffer, sizeof(ungranted_buffer));
    R[5] = (uint64_t)native_call(172, session[0], 0, 0);
    if (R[3] != (uint64_t)-13L || R[4] != (uint64_t)-13L || R[5] != (uint64_t)-13L) return 41;
    if (session[1] & RX_POLL_SESSION) {
        R[12] = (uint64_t)native_call(173, session[0], 2, 0);
        if (R[12] != (uint64_t)-13L) return 48;
    }
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    if (!(session[1] & RX_IRQ_SESSION)) return 55;
    IRQ_PEER[0] = 0x4a4e495251503031ULL;
    IRQ_PEER[1] = session[0];
    IRQ_PEER[2] = (uint64_t)native_call(174, session[0], 0, 0);
    IRQ_PEER[3] = (uint64_t)native_call(175, session[0], 0, 0);
    if (IRQ_PEER[2] != (uint64_t)-13L || IRQ_PEER[3] != (uint64_t)-13L) return 56;
    if (session[1] & RX_IRQ_RACE_SESSION) {
        IRQ_PEER[12] = (uint64_t)native_call(176, session[0], 0, 0);
        if (IRQ_PEER[12] != (uint64_t)-13L) return 67;
    }
#endif
    unsigned char reply = 0xa3;
    R[1] = 3;
    if (pipe_send(REPLY_FD, &reply, 1)) return 42;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    int quiet = quiet_ticks();
    if (quiet) return quiet;
#endif
    for (unsigned seq = 0; seq < FRAME_COUNT; ++seq) {
        unsigned char *received = jnox_rx_peer_frames[seq];
        unsigned len = frame_length(seq);
        if (pipe_receive(DATA_FD, received, len)) return 43;
        uint64_t nonce = seq ? R[8] : read_be(received + 22, 8);
        if (!valid_frame(received, len, seq, R[10], nonce)) return 44;
        if (!seq) {
            R[8] = nonce;
            R[1] = 4;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
            int tick = first_frame_tick();
            if (tick) return tick;
#endif
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
