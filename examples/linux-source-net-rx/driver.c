#include "protocol.h"
#include <time.h>
#include <unistd.h>

extern int jnox_packet_context_enter_v1(unsigned int xmit_more);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern long jnox_net_setup_queues(unsigned mode, unsigned long long receipt[10]);
extern void jnox_net_forget_queues(void);
extern int jnox_original_rx_init(void);
extern long jnox_rx_start_v1(void);
extern int jnox_original_rx_read(void *out, unsigned int capacity);
extern unsigned long jnox_rx_posted_address_v1(unsigned int id);

volatile uint64_t jnox_rx_driver_result[32];
volatile uint64_t jnox_rx_owner_controls[4];
#define R jnox_rx_driver_result
#define CTRL jnox_rx_owner_controls
#define NS_PER_S 1000000000ULL
#define VERSION_1 (1ULL << 32)
#define MAC (1U << 5)
#define MTU (1U << 3)
static unsigned char ethernet[1514];

#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
extern int jnox_net_irq_queue_receipt_v1(unsigned long long out[10]);
extern void jnox_original_rx_irq_stats_v1(unsigned long long out[8]);
extern unsigned long long jnox_rx_catchups_v1(void);
unsigned long long jnox_rx_irq_phase[4][8];
uint64_t jnox_rx_irq_kernel[3][27];
volatile uint64_t jnox_rx_irq_owner[32];
#define IRQ_OWNER jnox_rx_irq_owner

static int snapshot_irq(unsigned int phase)
{
    uint64_t *k = jnox_rx_irq_kernel[phase];
    unsigned long long *a = jnox_rx_irq_phase[phase];
    jnox_original_rx_irq_stats_v1(a);
    for (unsigned int i = 0; i < 27; ++i) {
        long value = native_call(175, R[3], i, 0);
        if (value < 0) return 57;
        k[i] = (uint64_t)value;
    }
    IRQ_OWNER[3] = jnox_rx_catchups_v1();
    if (k[19] != R[3] || k[20] != 63 || k[22]) return 58;
    if (!phase)
        return (a[3] || a[4] || a[5] || a[6] || k[2] || k[6] || k[7] || k[21]) ? 59 : 0;
#ifdef JNOX_NATIVE_RX_IRQ_RACE_FIXTURE
    if (k[5] != 1) return 62;
#endif
    if (!a[0] || a[1] || !a[3] || a[3] + IRQ_OWNER[3] != a[6] || a[5] > k[21] ||
        a[3] != k[7] + k[9] || IRQ_OWNER[3] != k[10] ||
        !k[6] || !k[7] || !k[16] || k[0] != k[14]) return 60;
    return 0;
}
#endif

#ifdef JNOX_NATIVE_RX_POLL_FIXTURE
#include "../../drivers/linux-compat/packet/include/jnox_rx_skb.h"
extern void jnox_original_rx_poll_stats_v1(unsigned long long out[8]);
extern void jnox_rx_napi_stats_v1(unsigned long long out[8]);

unsigned long long jnox_rx_poll_receipt[8];
unsigned long long jnox_rx_skb_receipt[8];
unsigned long long jnox_rx_napi_receipt[8];

/* Publish a consistent set while this worker owns the token. No other
 * process can dispatch the callback or dequeue an skb between these reads. */
static int snapshot_poll(unsigned int expected_frames)
{
    const unsigned long long *poll = jnox_rx_poll_receipt;
    const unsigned long long *skb = jnox_rx_skb_receipt;
    const unsigned long long *napi = jnox_rx_napi_receipt;
    jnox_original_rx_poll_stats_v1(jnox_rx_poll_receipt);
    jnox_rx_skb_stats_v1(jnox_rx_skb_receipt);
    jnox_rx_napi_stats_v1(jnox_rx_napi_receipt);
    if (!poll[0] || poll[1] != 8 || poll[3] != expected_frames || poll[5] != expected_frames ||
        napi[0] != 1 || napi[1] != 1 || napi[2] != poll[2] || napi[3] != poll[4])
        return 35;
    for (unsigned i = 0; i < 6; ++i) {
        if (skb[i] != expected_frames) return 36;
    }
    if (skb[6] || skb[7] || (expected_frames && !poll[7])) return 37;
    return 0;
}
#endif

static int configure(void)
{
    long low = native_call(157, 0, 0, 0), high = native_call(157, 1, 0, 0);
    R[22] = (uint64_t)low;
    R[23] = (uint64_t)high;
    if (low < 0 || high < 0 || !(low & MAC) || !(high & 1)) return 12;
    for (unsigned i = 0; i < 6; ++i) {
        long byte = native_call(158, i, 1, 0);
        if (byte < 0 || byte > 255) return 13;
        R[5] = (R[5] << 8) | (unsigned)byte;
    }
    unsigned mtu = 0, reads = 0, diagnostics = 0;
    unsigned long long features = VERSION_1 | ((unsigned)low & (MAC | MTU));
    R[18] = (uint64_t)jnox_net_validate(&features, 0, &mtu, &reads, &diagnostics);
    R[4] = features;
    R[19] = mtu;
    R[20] = reads;
    R[21] = diagnostics;
    if (R[18] || reads != !!(low & MTU) || diagnostics ||
        features != (VERSION_1 | MAC | ((low & MTU) && mtu >= 68 ? MTU : 0))) return 14;
    if (native_call(159, (uint32_t)features, features >> 32, 0)) return 15;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    return 0;
}

static int initialized_queues(void)
{
    unsigned long long queue[10] = {0};
    if (jnox_net_irq_queue_receipt_v1(queue)) return 16;
#else
    unsigned long long queue[10] = {0};
    if (jnox_net_setup_queues(0, queue)) return 16;
#endif
    R[3] = queue[0];
    R[24] = queue[1];
    R[25] = queue[2];
    if (!queue[0] || queue[7] != 3 || queue[8] || queue[9]) return 17;
    for (unsigned i = 0; i < 2; ++i) {
        if (queue[3 + i] != 32 || queue[5 + i] < 32 ||
            queue[1 + i] != ((queue[5 + i] << 32) | (32ULL << 16) | 1)) return 18;
    }
    R[26] = (uint64_t)native_call(160, 0, 0, 0);
    return R[26] == 11 ? 0 : 19;
}

static int start_rx(void)
{
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    R[6] = (uint64_t)jnox_original_rx_init();
    if (R[6]) return 22;
    int err = initialized_queues();
    if (err) return err;
#endif
    uint64_t token = R[3];
#ifndef JNOX_NATIVE_RX_IRQ_FIXTURE
    CTRL[0] = (uint64_t)native_call(170, token, 0, 0);
#endif
    CTRL[1] = (uint64_t)native_call(171, token, (uintptr_t)ethernet, 1529);
    CTRL[2] = (uint64_t)native_call(171, token, 0, 1530);
    if (CTRL[1] != (uint64_t)-90L || CTRL[2] != (uint64_t)-14L) return 20;
#ifndef JNOX_NATIVE_RX_IRQ_FIXTURE
    if (CTRL[0] != (uint64_t)-11L) return 20;
#endif
    R[30] = (uint64_t)native_call(163, token, 0, 0);
    R[31] = (uint64_t)native_call(167, token, (uintptr_t)ethernet, sizeof(ethernet));
    if (R[30] != (uint64_t)-95L || R[31] != (uint64_t)-95L) return 21;
#ifndef JNOX_NATIVE_RX_IRQ_FIXTURE
    R[6] = (uint64_t)jnox_original_rx_init();
    if (R[6]) return 22;
#endif
    unsigned long address = jnox_rx_posted_address_v1(0);
    if (!address) return 23;
    CTRL[3] = (uint64_t)native_call(171, token, address + 1, 1530);
    if (CTRL[3] != (uint64_t)-16L) return 24;
    R[7] = (uint64_t)jnox_rx_start_v1();
    R[8] = (uint64_t)native_call(160, 0, 0, 0);
    return R[7] || R[8] != 15 ? 25 : 0;
}

#ifndef JNOX_NATIVE_RX_IRQ_FIXTURE
static int monotonic_ns(uint64_t *value)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return -1;
    *value = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
    return 0;
}
#endif

static int get_frame(void)
{
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    for (;;) {
        int n = jnox_original_rx_read(ethernet, sizeof(ethernet));
        if (n != -11) return n;
        ++IRQ_OWNER[4]; /* Adapter consumed an event with no deliverable SKB. */
    }
#else
    uint64_t start;
    if (monotonic_ns(&start)) return -1;
    for (;;) {
        int n = jnox_original_rx_read(ethernet, sizeof(ethernet));
        if (n != -11) return n;
        uint64_t now;
        if (monotonic_ns(&now) || now - start >= 10 * NS_PER_S) return -1;
        uint64_t wake = now + 5000000ULL;
        struct timespec t = {(time_t)(wake / NS_PER_S), (long)(wake % NS_PER_S)};
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0)) return -1;
    }
#endif
}

static int interrupt_status(void)
{
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    return 0; /* Only the routed EL1 IRQ handler acknowledges the NIC. */
#else
    long pending = native_call(168, R[3], 0, 0);
    if (pending < 0 || (pending & ~3L)) return -1;
    R[29] |= (unsigned)pending;
    return 0;
#endif
}

static int deliver(void)
{
    uint64_t session[2] = {R[3], R[5]};
#ifdef JNOX_NATIVE_RX_POLL_FIXTURE
    session[1] |= RX_POLL_SESSION;
#endif
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    session[1] |= RX_IRQ_SESSION;
    IRQ_OWNER[0] = 0x4a4e4952514f3031ULL;
    IRQ_OWNER[1] = R[3];
#ifdef JNOX_NATIVE_RX_IRQ_RACE_FIXTURE
    session[1] |= RX_IRQ_RACE_SESSION;
    IRQ_OWNER[11] = 1;
    IRQ_OWNER[12] = (uint64_t)native_call(176, R[3], 0, 0);
    if (IRQ_OWNER[12]) return 63;
#endif
#endif
    unsigned char reply = 0;
    if (pipe_send(DATA_FD, session, sizeof(session)) ||
        pipe_receive(REPLY_FD, &reply, 1) || reply != 0xa3) return 26;
#ifdef JNOX_NATIVE_RX_POLL_FIXTURE
    int check = snapshot_poll(0);
    if (check) return check;
#endif
    R[1] = 3;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    IRQ_OWNER[7] = (uint64_t)native_call(174, R[3], 0, 1);
    IRQ_OWNER[8] = (uint64_t)native_call(174, R[3], (uint64_t)-1, 0);
    IRQ_OWNER[9] = (uint64_t)native_call(175, R[3], 0, 1);
    IRQ_OWNER[10] = (uint64_t)native_call(175, R[3], 27, 0);
    for (unsigned int i = 7; i <= 10; ++i)
        if (IRQ_OWNER[i] != (uint64_t)-22L) return 61;
    if ((check = snapshot_irq(0))) return check;
    IRQ_OWNER[2] = 1;
#endif
    (void)write(STDOUT_FILENO, "NET RX READY\n", sizeof("NET RX READY\n") - 1);
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    reply = 0xb1;
    if (pipe_send(DATA_FD, &reply, 1)) return 49;
#endif
    for (unsigned seq = 0; seq < FRAME_COUNT; ++seq) {
        int n = get_frame();
        if (n != (int)frame_length(seq)) return 27;
        uint64_t nonce = seq ? R[9] : read_be(ethernet + 22, 8);
        if (!valid_frame(ethernet, (unsigned)n, seq, R[5], nonce)) return 28;
        if (!seq) {
            R[9] = nonce;
            R[1] = 4;
        }
        ++R[10];
        R[11] += (unsigned)n;
        R[13] = seq;
        if (interrupt_status() || pipe_send(DATA_FD, ethernet, (unsigned)n)) return 29;
        ++R[12];
    }
    if (pipe_receive(REPLY_FD, &reply, 1) || reply != 0xa6) return 30;
    R[14] = reply;
#ifdef JNOX_NATIVE_RX_POLL_FIXTURE
    check = snapshot_poll(FRAME_COUNT);
    if (check) return check;
#endif
    R[1] = 6;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    if ((check = snapshot_irq(1))) return check;
    IRQ_OWNER[2] = 2;
#endif
    (void)write(STDOUT_FILENO, "NET RX WAIT ACK\n", sizeof("NET RX WAIT ACK\n") - 1);
    int n = get_frame();
    if (n != ACK_SIZE || !valid_ack(ethernet, (unsigned)n, R[5], R[9]) || interrupt_status()) return 31;
    R[27] = (unsigned)n;
    if (pipe_send(DATA_FD, ethernet, (unsigned)n) ||
        pipe_receive(REPLY_FD, &reply, 1) || reply != 0xa7) return 32;
#ifdef JNOX_NATIVE_RX_POLL_FIXTURE
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
    if ((check = snapshot_irq(2))) return check;
    IRQ_OWNER[2] = 3;
    IRQ_OWNER[3] = jnox_rx_catchups_v1();
#endif
    return snapshot_poll(FRAME_COUNT + 1);
#else
    return 0;
#endif
}

int main(void)
{
    R[0] = 0x4a4e525844525631ULL;
    R[1] = 1;
    if (native_call(156, 1, 0, 0)) {
        R[2] = 10;
        return 10;
    }
    int rc = jnox_packet_context_enter_v1(0);
    if (rc) {
        rc = 11;
    } else {
        rc = configure();
        jnox_packet_context_leave_v1();
    }
    if (!rc) rc = start_rx();
    if (!rc) rc = deliver();
    R[15] = (uint64_t)native_call(156, 0, 0, 0);
    if (!R[15]) jnox_net_forget_queues();
    if (!rc && R[15]) rc = 33;
    if (!rc) {
        R[16] = (uint64_t)native_call(171, R[3], 0, 1530);
        R[17] = (uint64_t)native_call(172, R[3], 0, 0);
        R[28] = (uint64_t)native_call(160, 0, 0, 0);
        if (R[16] != (uint64_t)-116L || R[17] != (uint64_t)-116L || R[28] != (uint64_t)-19L) rc = 34;
#ifdef JNOX_NATIVE_RX_IRQ_FIXTURE
        IRQ_OWNER[5] = (uint64_t)native_call(174, R[3], 0, 0);
        IRQ_OWNER[6] = (uint64_t)native_call(175, R[3], 0, 0);
        jnox_original_rx_irq_stats_v1(jnox_rx_irq_phase[3]);
        if (IRQ_OWNER[5] != (uint64_t)-116L || IRQ_OWNER[6] != (uint64_t)-116L) rc = 50;
#endif
    }
    R[2] = (unsigned)rc;
    if (rc) return rc;
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET RX DRIVER PASS\n", sizeof("NET RX DRIVER PASS\n") - 1);
    return 0;
}
