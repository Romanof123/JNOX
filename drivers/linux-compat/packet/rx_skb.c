// SPDX-License-Identifier: GPL-2.0-or-later
/* Socketless, linear small-RX profile. The page reference returned in the
 * used cookie transfers to build_skb and remains live until native delivery. */
#include <linux/etherdevice.h>
#include <linux/if_arp.h>
#include <linux/netdevice.h>
#include <linux/sched/clock.h>
#include <linux/timekeeping.h>
#include <uapi/linux/time_types.h>
#include <net/flow_dissector.h>
#include <jnox_rx_page_api.h>
#include <jnox_rx_skb.h>

enum { DEPTH = 32 };
enum state { FREE, BUILT, DELIVERED };
struct rx_slot {
    struct sk_buff skb;
    struct page *backing;
    enum state state;
};
static struct rx_slot slots[DEPTH];
static struct rx_slot *fifo[DEPTH];
static unsigned int read_pos, count;
static struct {
    unsigned long long built, appended, classified, queued, read, released, dropped;
} stats;

static struct rx_slot *find_slot(const struct sk_buff *skb)
{
    for (unsigned int i = 0; i < DEPTH; ++i)
        if (&slots[i].skb == skb && slots[i].state != FREE)
            return &slots[i];
    __builtin_trap();
}

struct sk_buff *build_skb(void *data, unsigned int size)
{
    if (size <= SKB_DATA_ALIGN(sizeof(struct skb_shared_info)) ||
        !jnox_rx_span_v1(data, size))
        return NULL;
    for (unsigned int i = 0; i < DEPTH; ++i) {
        if (slots[i].state != FREE && slots[i].skb.head == data)
            __builtin_trap();
    }
    for (unsigned int i = 0; i < DEPTH; ++i) {
        struct rx_slot *s = &slots[i];
        if (s->state != FREE)
            continue;
        struct sk_buff *skb = &s->skb;
        memset(skb, 0, sizeof(*skb));
        skb->head = skb->data = data;
        skb_reset_tail_pointer(skb);
        skb_set_end_offset(skb, SKB_WITH_OVERHEAD(size));
        skb->truesize = SKB_TRUESIZE(skb_end_offset(skb));
        skb->head_frag = 1;
        skb->mac_header = (typeof(skb->mac_header))~0U;
        skb->transport_header = (typeof(skb->transport_header))~0U;
        refcount_set(&skb->users, 1);
        memset(skb_shinfo(skb), 0, sizeof(struct skb_shared_info));
        atomic_set(&skb_shinfo(skb)->dataref, 1);
        s->backing = virt_to_head_page(data);
        s->state = BUILT;
        ++stats.built;
        return skb;
    }
    return NULL; /* upstream still owns the page reference on failure */
}

void *skb_put(struct sk_buff *skb, unsigned int len)
{
    if (find_slot(skb)->state != BUILT || skb_is_nonlinear(skb) ||
        skb_tailroom(skb) < 0 || len > (unsigned int)skb_tailroom(skb))
        __builtin_trap();
    void *tail = __skb_put(skb, len);
    ++stats.appended;
    return tail;
}

__be16 eth_type_trans(struct sk_buff *skb, struct net_device *dev)
{
    if (find_slot(skb)->state != BUILT || !dev || !dev->dev_addr ||
        dev->type != ARPHRD_ETHER || dev->addr_len != ETH_ALEN ||
        skb_is_nonlinear(skb) || skb->len < ETH_HLEN)
        __builtin_trap();
    skb->dev = dev;
    skb_reset_mac_header(skb);
    const struct ethhdr *eth = eth_hdr(skb);
    if (is_multicast_ether_addr(eth->h_dest))
        skb->pkt_type = is_broadcast_ether_addr(eth->h_dest) ?
                        PACKET_BROADCAST : PACKET_MULTICAST;
    else if (!ether_addr_equal(dev->dev_addr, eth->h_dest))
        skb->pkt_type = PACKET_OTHERHOST;
    skb_pull_inline(skb, ETH_HLEN);
    ++stats.classified;
    if (eth_proto_is_802_3(eth->h_proto))
        return eth->h_proto;
    if (skb->len >= 2 && skb->data[0] == 0xff && skb->data[1] == 0xff)
        return htons(ETH_P_802_3);
    return htons(ETH_P_802_2);
}

static void release(struct rx_slot *s)
{
    if (refcount_read(&s->skb.users) != 1 ||
        atomic_read(&skb_shinfo(&s->skb)->dataref) != 1 ||
        s->skb.cloned || s->skb.destructor || skb_is_nonlinear(&s->skb))
        __builtin_trap();
    put_page(s->backing);
    s->backing = NULL;
    s->state = FREE;
    ++stats.released;
}

void consume_skb(struct sk_buff *skb)
{
    if (skb) {
        struct rx_slot *s = find_slot(skb);
        if (s->state != BUILT)
            __builtin_trap();
        release(s);
    }
}

void sk_skb_reason_drop(struct sock *sk, struct sk_buff *skb,
                        enum skb_drop_reason reason)
{
    if (sk)
        __builtin_trap();
    consume_skb(skb);
    if (skb && reason != SKB_CONSUMED)
        ++stats.dropped;
}

/* The shared cleanup helpers can release a live, undelivered RX SKB.
 * This profile has no registered TX cookies. Never consume unknown or
 * already-delivered storage through the TX compatibility entry. */
int jnox_packet_retire_skb_v1(struct sk_buff *skb, unsigned int reason)
{
    for (unsigned int i = 0; i < DEPTH; ++i) {
        if (skb != &slots[i].skb)
            continue;
        if (slots[i].state != BUILT)
            return -EBUSY;
        sk_skb_reason_drop(NULL, skb, reason);
        return 0;
    }
    return -ENOENT;
}

extern int rx_clock_gettime(int, struct __kernel_timespec *) asm("clock_gettime");

ktime_t ktime_get(void)
{
    struct __kernel_timespec now;
    if (rx_clock_gettime(CLOCK_MONOTONIC, &now) ||
        now.tv_sec < 0 || now.tv_nsec < 0 || now.tv_nsec >= NSEC_PER_SEC ||
        now.tv_sec > (S64_MAX - now.tv_nsec) / NSEC_PER_SEC)
        __builtin_trap();
    return now.tv_sec * NSEC_PER_SEC + now.tv_nsec;
}

u64 sched_clock(void)
{
    return ktime_get();
}

/* Readable in the stopped service image. Record format metadata even
 * when the full Linux variadic formatter is unavailable. */
static struct {
    unsigned long long netdev_calls, printk_calls;
    const struct _ddebug *site;
    const struct net_device *dev;
    const char *format;
    char text[128];
} volatile diagnostics;

void __dynamic_netdev_dbg(struct _ddebug *site, const struct net_device *dev,
                          const char *fmt, ...)
{
    if (!site || !dev || !fmt)
        __builtin_trap();
    diagnostics.site = site;
    diagnostics.dev = dev;
    diagnostics.format = fmt;
    ++diagnostics.netdev_calls;
}

int _printk(const char *fmt, ...)
{
    unsigned int n = 0;
    bool conversion = false;
    ++diagnostics.printk_calls;
    diagnostics.format = fmt;
    diagnostics.text[0] = 0;
    if (!fmt)
        return -EINVAL;
    while (n + 1 < sizeof(diagnostics.text) && fmt[n]) {
        conversion |= fmt[n] == '%';
        diagnostics.text[n] = fmt[n];
        ++n;
    }
    diagnostics.text[n] = 0;
    if (fmt[n])
        return -ENOSPC;
    return conversion ? -EOPNOTSUPP : (int)n;
}

/* The Linux basic dissector selects control and basic keys at their
 * native offsets. The no-GSO profile never invokes flow dissection. */
struct flow_dissector flow_keys_basic_dissector = {
    .used_keys = BIT_ULL(FLOW_DISSECTOR_KEY_CONTROL) | BIT_ULL(FLOW_DISSECTOR_KEY_BASIC),
    .offset = {
        [FLOW_DISSECTOR_KEY_CONTROL] = offsetof(struct flow_keys_basic, control),
        [FLOW_DISSECTOR_KEY_BASIC] = offsetof(struct flow_keys_basic, basic),
    },
};

gro_result_t gro_receive_skb(struct gro_node *gro, struct sk_buff *skb)
{
    struct rx_slot *s = find_slot(skb);
    if (!gro || s->state != BUILT || !skb->dev ||
        container_of(gro, struct napi_struct, gro)->dev != skb->dev ||
        (skb->dev->features & NETIF_F_GRO) || skb->ip_summed != CHECKSUM_NONE ||
        skb->cb[0] || skb->cb[1] || skb_is_gso(skb) || skb_is_nonlinear(skb) ||
        skb_mac_header(skb) + ETH_HLEN != skb->data || count == DEPTH)
        __builtin_trap();
    skb_reset_network_header(skb);
    skb_reset_mac_len(skb);
    fifo[(read_pos + count) % DEPTH] = s;
    ++count;
    s->state = DELIVERED;
    ++stats.queued;
    return GRO_NORMAL;
}

unsigned int jnox_rx_skb_pending_v1(void)
{
    return count;
}

void jnox_rx_skb_stats_v1(unsigned long long out[8])
{
    out[0] = stats.built;
    out[1] = stats.appended;
    out[2] = stats.classified;
    out[3] = stats.queued;
    out[4] = stats.read;
    out[5] = stats.released;
    out[6] = count;
    out[7] = stats.dropped;
}

int jnox_rx_skb_read_v1(void *out, unsigned int capacity)
{
    if (!out || !capacity)
        return -EINVAL;
    if (!count)
        return -EAGAIN;
    struct rx_slot *s = fifo[read_pos];
    struct sk_buff *skb = &s->skb;
    unsigned int len = skb->len + ETH_HLEN;
    if (s->state != DELIVERED || !jnox_rx_span_v1(skb_mac_header(skb), len))
        __builtin_trap();
    if (capacity < len)
        return -EMSGSIZE;
    memcpy(out, skb_mac_header(skb), len);
    fifo[read_pos] = NULL;
    read_pos = (read_pos + 1) % DEPTH;
    --count;
    release(s);
    ++stats.read;
    return len;
}
