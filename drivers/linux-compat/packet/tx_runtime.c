// SPDX-License-Identifier: GPL-2.0-or-later
/* Native single-threaded, socketless, linear SKB transmit profile. */
#include <linux/dynamic_debug.h>
#include <linux/jiffies.h>
#include <linux/netdevice.h>
#include <linux/refcount.h>
#include <uapi/linux/time_types.h>
#include <net/dst.h>
#include <net/xdp.h>
#include <net/xdp_sock_drv.h>
#include <jnox_packet_tx_runtime.h>

/* SDK clock_gettime ABI, without mixing SDK and Linux time headers. */
extern int tx_clock_gettime(int, struct __kernel_timespec *) asm("clock_gettime");
extern int jnox_packet_retire_skb_v1(struct sk_buff *, unsigned int);

unsigned long volatile __cacheline_aligned_in_smp __jiffy_arch_data jiffies;
static struct netdev_queue *wake_pending;
static struct _ddebug *last_debug;
static unsigned long rate_epoch;
static unsigned int rate_count;
volatile unsigned int jnox_packet_tx_fault_v1;
volatile unsigned long jnox_packet_tx_debug_count_v1;

static __noreturn void fault(unsigned int reason)
{
    WRITE_ONCE(jnox_packet_tx_fault_v1, reason);
    __builtin_trap();
}

int jnox_packet_tx_clock_v1(void)
{
    struct __kernel_timespec now;
    if (tx_clock_gettime(CLOCK_MONOTONIC, &now))
        return -EIO;
    if (now.tv_sec < 0 || now.tv_nsec < 0 || now.tv_nsec >= NSEC_PER_SEC)
        return -ERANGE;
    WRITE_ONCE(jiffies, (unsigned long)now.tv_sec * HZ +
               (unsigned long)now.tv_nsec * HZ / NSEC_PER_SEC);
    return 0;
}

int net_ratelimit(void)
{
    if (jnox_packet_tx_clock_v1())
        fault(JNOX_TX_CLOCK);
    if (time_after_eq(jiffies, rate_epoch + 5UL * HZ)) {
        rate_epoch = jiffies;
        rate_count = 0;
    }
    if (rate_count == 10)
        return 0;
    ++rate_count;
    return 1;
}

void __dynamic_pr_debug(struct _ddebug *descriptor, const char *fmt, ...)
{
    if (!descriptor || !fmt)
        fault(JNOX_TX_DEBUG);
    /* Identify the exact stock callsite without dereferencing variadic
     * Linux formatter arguments as native SDK objects. */
    WRITE_ONCE(last_debug, descriptor);
    ++jnox_packet_tx_debug_count_v1;
}

void napi_consume_skb(struct sk_buff *skb, int budget)
{
    if (budget < 0 || (skb && jnox_packet_retire_skb_v1(skb, SKB_CONSUMED)))
        fault(JNOX_TX_COOKIE);
}

void dev_kfree_skb_any_reason(struct sk_buff *skb, enum skb_drop_reason reason)
{
    if (skb && jnox_packet_retire_skb_v1(skb, reason))
        fault(JNOX_TX_COOKIE);
}

/* BQL's Linux enqueue side runs inline. This profile uses one fixed byte
 * limit configured by the frame owner and has no adaptive stall monitor. */
void dql_completed(struct dql *d, unsigned int count)
{
    if (!d || !d->limit || d->limit > DQL_MAX_LIMIT ||
        d->min_limit != d->limit || d->max_limit != d->limit ||
        d->stall_thrs || count > d->num_queued - d->num_completed)
        fault(JNOX_TX_BQL);
    d->num_completed += count;
    WRITE_ONCE(d->adj_limit, d->num_completed + d->limit);
}

/* A wakeup coalesces until the event loop takes it. There is one TX queue
 * and no Linux qdisc or NAPI poll worker in this service. */
void netif_schedule_queue(struct netdev_queue *q)
{
    if (!q || (wake_pending && wake_pending != q))
        fault(JNOX_TX_QUEUE);
    wake_pending = q;
}

bool jnox_packet_tx_take_wake_v1(struct netdev_queue *q)
{
    if (!q || !wake_pending)
        return false;
    if (wake_pending != q)
        fault(JNOX_TX_QUEUE);
    wake_pending = NULL;
    return true;
}

#ifndef JNOX_PACKET_RX_POLL_ABI
bool napi_schedule_prep(struct napi_struct *n)
{
    if (n && (napi_disable_pending(n) || test_bit(NAPI_STATE_DISABLED, &n->state)))
        return false;
    fault(JNOX_TX_NAPI);
}

void __napi_schedule(struct napi_struct *n)
{
    fault(JNOX_TX_NAPI);
}

#endif

void skb_clone_tx_timestamp(struct sk_buff *skb)
{
    if (!skb || skb->sk || skb_shinfo(skb)->tx_flags)
        fault(JNOX_TX_TIMESTAMP);
}

void skb_tstamp_tx(struct sk_buff *skb, struct skb_shared_hwtstamps *hw)
{
    fault(JNOX_TX_TIMESTAMP);
}

int skb_copy_ubufs(struct sk_buff *skb, gfp_t mask)
{
    return skb ? -EOPNOTSUPP : -EINVAL;
}

void dst_release(struct dst_entry *dst)
{
    if (dst)
        fault(JNOX_TX_ROUTE);
}

void nf_conntrack_destroy(struct nf_conntrack *ct)
{
    fault(JNOX_TX_CONNTRACK);
}

void refcount_warn_saturate(refcount_t *ref, enum refcount_saturation_type why)
{
    if (ref)
        refcount_set(ref, REFCOUNT_SATURATED);
    fault(JNOX_TX_REFCOUNT);
}

void xdp_return_frame(struct xdp_frame *frame)
{
    fault(JNOX_TX_XDP);
}

void xsk_tx_completed(struct xsk_buff_pool *pool, u32 count)
{
    fault(JNOX_TX_XSK);
}
