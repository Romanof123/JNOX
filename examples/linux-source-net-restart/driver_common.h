#ifndef JNOX_SOURCE_NET_RESTART_DRIVER_H
#define JNOX_SOURCE_NET_RESTART_DRIVER_H
#include "protocol.h"
extern int jnox_packet_context_enter_v1(unsigned int);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern int jnox_original_rx_init(void);
extern int jnox_net_irq_queue_receipt_v1(unsigned long long out[10]);
extern long jnox_rx_start_v1(void);
extern int jnox_original_rx_read(void *, unsigned int);
extern unsigned long jnox_rx_posted_address_v1(unsigned int);
extern void jnox_original_rx_irq_stats_v1(unsigned long long out[8]);
extern void jnox_original_rx_poll_stats_v1(unsigned long long out[8]);
extern void jnox_rx_skb_stats_v1(unsigned long long out[8]);
extern void jnox_net_forget_queues(void);

volatile uint64_t jnox_restart_driver[32];
unsigned long long jnox_restart_callback[8], jnox_restart_poll[8], jnox_restart_skb[8];
unsigned long jnox_restart_grant_addresses[32];
#define D jnox_restart_driver
static unsigned char frame[1514];
static struct session sess;

static int configure(void)
{
    long low = native_call(157,0,0,0), high = native_call(157,1,0,0);
    if (low < 0 || high < 0 || !(low & 32) || !(high & 1)) return 12;
    for (unsigned i = 0; i < 6; ++i) {
        long b = native_call(158,i,1,0);
        if (b < 0 || b > 255) return 13;
        sess.mac = sess.mac << 8 | (unsigned)b;
    }
    unsigned mtu=0, reads=0, diagnostics=0;
    unsigned long long features = (1ULL<<32) | ((unsigned)low & 40u);
    if (jnox_net_validate(&features,0,&mtu,&reads,&diagnostics) ||
        diagnostics || reads != !!(low & 8) ||
        features != ((1ULL<<32)|32|((low & 8) && mtu>=68 ? 8 : 0))) return 14;
    D[8] = features;
    D[9] = mtu;
    return native_call(159,(uint32_t)features,features>>32,0) ? 15 : 0;
}

static int start_device(void)
{
    if (native_call(156,1,0,0)) return 10;
    if (jnox_packet_context_enter_v1(0)) return 11;
    int rc = configure();
    jnox_packet_context_leave_v1();
    if (rc) return rc;
    unsigned long long q[10] = {0};
    if (jnox_original_rx_init() || jnox_net_irq_queue_receipt_v1(q) ||
        !q[0] || q[3]!=32 || q[4]!=32 || q[7]!=3 || q[8] || q[9]) return 16;
    sess.token = q[0];
    D[3] = sess.token;
    D[4] = sess.mac;
    for (unsigned i=0; i<32; ++i) {
        jnox_restart_grant_addresses[i] = jnox_rx_posted_address_v1(i);
        if (!jnox_restart_grant_addresses[i]) return 17;
    }
    if (jnox_rx_start_v1() || native_call(160,0,0,0)!=15) return 18;
    return 0;
}

static int receive_frame(void)
{
    int n;
    do { n = jnox_original_rx_read(frame,sizeof(frame)); } while (n == -11);
    return n;
}

static int callbacks(unsigned int expected)
{
    jnox_original_rx_irq_stats_v1(jnox_restart_callback);
    jnox_original_rx_poll_stats_v1(jnox_restart_poll);
    jnox_rx_skb_stats_v1(jnox_restart_skb);
    if (!jnox_restart_callback[0] || !jnox_restart_callback[3] ||
        !jnox_restart_poll[0] || jnox_restart_poll[3]!=expected ||
        jnox_restart_poll[5]!=expected || jnox_restart_skb[6] || jnox_restart_skb[7]) return 19;
    for (unsigned i=0; i<6; ++i) if (jnox_restart_skb[i]!=expected) return 19;
    return 0;
}
#endif
