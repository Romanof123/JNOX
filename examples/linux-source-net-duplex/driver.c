#include "protocol.h"
#include <time.h>

extern int jnox_packet_context_enter_v1(unsigned int);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern int jnox_lifecycle_probe_v1(void), jnox_lifecycle_open_v1(void);
extern int jnox_lifecycle_close_v1(void), jnox_lifecycle_remove_v1(void);
extern void jnox_lifecycle_stats_v1(unsigned long long out[12]);
extern unsigned long jnox_packet_token(void), jnox_rx_posted_address_v1(unsigned int);
extern int jnox_original_rx_read(void *, unsigned int);
extern int jnox_original_tx_init(void);
extern int jnox_original_tx_frame(const unsigned char *, unsigned int, unsigned int);
extern int jnox_original_tx_defer(const unsigned char *, unsigned int, unsigned int);
extern int jnox_original_tx_flush(unsigned int), jnox_original_tx_complete(unsigned int);
extern void jnox_rx_vq_stats(unsigned long long out[8]), jnox_tx_vq_stats(unsigned long long out[8]);

volatile uint64_t jnox_duplex_driver[32];
unsigned long long jnox_duplex_lifecycle[DUPLEX_SNAPSHOTS][12];
unsigned long long jnox_duplex_rx_queue[DUPLEX_SNAPSHOTS][8];
unsigned long long jnox_duplex_tx_queue[DUPLEX_SNAPSHOTS][8];
static unsigned char inbound[1514], outbound[MAX_FRAME], held_reply[MAX_FRAME];
static struct duplex_session session;

static unsigned posted_rx(void)
{
    unsigned count = 0;
    for (unsigned i = 0; i < 32; ++i)
        if (jnox_rx_posted_address_v1(i))
            ++count;
    return count;
}

static void snapshot(enum duplex_snapshot stage)
{
    jnox_lifecycle_stats_v1(jnox_duplex_lifecycle[stage]);
    jnox_rx_vq_stats(jnox_duplex_rx_queue[stage]);
    jnox_tx_vq_stats(jnox_duplex_tx_queue[stage]);
}

static int negotiate(void)
{
    volatile uint64_t *d = jnox_duplex_driver;
    long low = native_call(157, 0, 0, 0);
    long high = native_call(157, 1, 0, 0);
    if (low < 0 || high < 0 || !(low & 32) || !(high & 1))
        return 11;
    for (unsigned i = 0; i < 6; ++i) {
        long value = native_call(158, i, 1, 0);
        if (value < 0 || value > 255)
            return 12;
        session.mac = (session.mac << 8) | (unsigned)value;
    }
    unsigned mtu = 0, reads = 0, diagnostics = 0;
    unsigned long long features = (1ULL << 32) | ((unsigned)low & 40u);
    if (jnox_net_validate(&features, 0, &mtu, &reads, &diagnostics) || diagnostics ||
        reads != !!(low & 8) || features != ((1ULL << 32) | 32 | ((low & 8) && mtu >= 68 ? 8 : 0)))
        return 13;
    d[3] = session.mac;
    d[11] = features;
    d[12] = mtu;
    d[13] = reads;
    return native_call(159, (uint32_t)features, features >> 32, 0) ? 14 : 0;
}

static int read_wire(void)
{
    int n;
    do {
        n = jnox_original_rx_read(inbound, sizeof(inbound));
    } while (n == -11);
    return n;
}

static int read_request(unsigned seq)
{
    int n = read_wire();
    if (n != (int)frame_length(seq))
        return 20;
    if (!seq)
        jnox_duplex_driver[4] = read_be(inbound + 22, 8);
    return valid_frame(inbound, n, seq, session.mac, jnox_duplex_driver[4]) ? 0 : 21;
}

/* The peer writes every reply byte. The driver verifies its envelope and
 * passes the pipe buffer unchanged to the registered Linux TX callback. */
static int pass_to_peer(unsigned seq, unsigned char *response)
{
    unsigned len = frame_length(seq);
    if (pipe_send(DATA_FD, inbound, len))
        return 22;
    ++jnox_duplex_driver[5];
    jnox_duplex_driver[9] += len;
    if (pipe_receive(REPLY_FD, response, len) ||
        !duplex_reply_header(response, seq, session.mac, jnox_duplex_driver[4]))
        return 23;
    ++jnox_duplex_driver[6];
    return 0;
}

static int reap(unsigned seq)
{
    struct timespec end, now;
    if (clock_gettime(CLOCK_MONOTONIC, &end))
        return 24;
    end.tv_sec += 5;
    for (;;) {
        int ret = jnox_original_tx_complete(seq);
        if (!ret)
            break;
        if (ret != -11 || clock_gettime(CLOCK_MONOTONIC, &now) ||
            now.tv_sec > end.tv_sec || (now.tv_sec == end.tv_sec && now.tv_nsec >= end.tv_nsec))
            return 25;
        now.tv_nsec += 1000000L;
        if (now.tv_nsec >= 1000000000L) {
            ++now.tv_sec;
            now.tv_nsec -= 1000000000L;
        }
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &now, 0))
            return 26;
    }
    ++jnox_duplex_driver[8];
    return 0;
}

static int submit(unsigned seq, const unsigned char *response)
{
    unsigned len = frame_length(seq);
    int ret;
    if (!seq) {
        ret = jnox_original_tx_defer(response, len, seq);
        jnox_duplex_driver[17] = ret;
    } else {
        ret = jnox_original_tx_frame(response, len, seq);
    }
    if (ret)
        return 27;
    ++jnox_duplex_driver[7];
    jnox_duplex_driver[10] += len;
    return seq ? reap(seq) : 0;
}

static int transfer(void)
{
    volatile uint64_t *d = jnox_duplex_driver;
    if (pipe_send(DATA_FD, &session, sizeof(session)) || expect_reply(0xa3))
        return 28;
    d[1] = 3;
    MARK("NET DUPLEX READY");
    int err = read_request(0);
    if (!err) err = pass_to_peer(0, outbound);
    if (!err) err = submit(0, outbound);
    if (err) return err;
    snapshot(DUPLEX_HELD);
    if (jnox_duplex_tx_queue[DUPLEX_HELD][6] != 1 || jnox_duplex_tx_queue[DUPLEX_HELD][1])
        return 29;
    d[1] = 4;
    MARK("NET DUPLEX TX HELD");

    err = read_request(1);
    if (!err) err = pass_to_peer(1, held_reply);
    if (err) return err;
    snapshot(DUPLEX_OVERLAP);
    if (jnox_duplex_tx_queue[DUPLEX_OVERLAP][6] != 1 || jnox_duplex_tx_queue[DUPLEX_OVERLAP][1] ||
        jnox_duplex_rx_queue[DUPLEX_OVERLAP][1] <= jnox_duplex_rx_queue[DUPLEX_HELD][1])
        return 30;
    d[1] = 5;
    MARK("NET DUPLEX OVERLAP");
    /* Request two releases the external observation gate. TX zero has
     * remained published and unnotified during a different RX delivery. */
    err = read_request(2);
    if (err) return err;
    d[18] = jnox_original_tx_flush(0);
    if (d[18]) return 31;
    err = reap(0);
    if (!err) err = submit(1, held_reply);
    if (err) return err;
    for (unsigned seq = 2; seq < FRAME_COUNT; ++seq) {
        if (seq != 2 && (err = read_request(seq))) return err;
        err = pass_to_peer(seq, outbound);
        if (!err) err = submit(seq, outbound);
        if (err) return err;
    }
    snapshot(DUPLEX_DELIVERED);
    if (d[5] != FRAME_COUNT || d[6] != FRAME_COUNT || d[7] != FRAME_COUNT ||
        d[8] != FRAME_COUNT || d[9] != d[10] || jnox_duplex_tx_queue[DUPLEX_DELIVERED][6])
        return 32;
    d[1] = 6;
    MARK("NET DUPLEX ACK");
    int n = read_wire();
    if (n != ACK_SIZE || !duplex_ack(inbound, n, session.mac, d[4]) ||
        pipe_send(DATA_FD, inbound, n) || expect_reply(0xa7))
        return 33;
    d[19] = n;
    return 0;
}

static int run(void)
{
    volatile uint64_t *d = jnox_duplex_driver;
    d[0] = 0x4455504c44525631ULL;  /* DUPLDRV1 */
    d[1] = 1;
    if (native_call(156, 1, 0, 0) || jnox_packet_context_enter_v1(0))
        return 10;
    int err = negotiate();
    jnox_packet_context_leave_v1();
    if (err) return err;
    if (jnox_lifecycle_probe_v1()) return 15;
    session.token = jnox_packet_token();
    d[2] = session.token;
    d[14] = native_call(160, 0, 0, 0);
    d[15] = posted_rx();
    d[28] = native_call(162, session.token, 0, 0);
    d[29] = native_call(162, session.token, 1, 0);
    snapshot(DUPLEX_PROBED);
    if (!session.token || d[14] != 15 || d[15] ||
        (uint32_t)d[28] != 0x200001u || (uint32_t)d[29] != 0x200001u)
        return 16;
    if (jnox_lifecycle_open_v1() || jnox_original_tx_init()) return 17;
    d[16] = posted_rx();
    d[30] = native_call(160, 0, 0, 0);
    snapshot(DUPLEX_OPENED);
    if (d[16] != 32 || d[30] != 15) return 18;
    err = transfer();
    if (err) return err;
    if (jnox_lifecycle_close_v1()) return 34;
    snapshot(DUPLEX_CLOSED);
    if (jnox_lifecycle_remove_v1()) return 35;
    snapshot(DUPLEX_REMOVED);
    d[20] = native_call(160, 0, 0, 0);
    d[21] = native_call(162, session.token, 0, 0);
    d[22] = native_call(162, session.token, 1, 0);
    d[23] = jnox_packet_token();
    d[24] = posted_rx();
    d[27] = native_call(166, session.token, 0, 0);
    if (d[20] != (uint64_t)-19L || d[21] != (uint64_t)-116L ||
        d[22] != (uint64_t)-116L || d[23] || d[24] || d[27] != (uint64_t)-116L)
        return 36;
    return 0;
}

int main(void)
{
    int rc = run();
    jnox_duplex_driver[31] = rc;
    if (!rc) {
        jnox_duplex_driver[1] = 100;
        MARK("NET DUPLEX DRIVER PASS");
    }
    return rc;
}
