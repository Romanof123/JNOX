/* SPDX-License-Identifier: GPL-2.0-or-later */
/* RX poll profile. Original queue allocation registers the NAPI callbacks;
 * this FIFO worker drives the registered RX callback with a bounded budget. */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <jnox_tx_virtqueue.h>
#include <jnox_rx_virtqueue.h>
#include <jnox_original_rx.h>
#include <linux/if_arp.h>

extern int virtnet_alloc_queues(struct virtnet_info *);
extern int virtnet_poll(struct napi_struct *, int);
extern int add_recvbuf_small(struct virtnet_info *, struct receive_queue *, gfp_t);
extern struct virtio_device *jnox_net_validated_device(void);
extern int jnox_rx_napi_poll_v1(struct napi_struct *, int);
extern int jnox_rx_napi_prepare_v1(struct net_device *);
extern int jnox_rx_skb_read_v1(void *, unsigned int);

enum { RX_DEPTH = 32, POLL_BUDGET = 8 };
#ifndef JNOX_PACKET_LIFECYCLE_ABI
static union {
    struct net_device alignment;
    unsigned char storage[offsetof(struct net_device, priv) + sizeof(struct virtnet_info)];
} rx_netdev;
static struct netdev_queue txq;
static unsigned char mac[ETH_ALEN] __aligned(2);
#endif
static struct receive_queue *bound_rq;
static unsigned long bound_epoch;
static unsigned long long receipt[8];

#ifdef JNOX_PACKET_RX_IRQ_ABI
extern bool jnox_rx_equal_name_v1(const char *, const char *);
extern int virtnet_find_vqs(struct virtnet_info *);
extern bool jnox_rx_napi_pending_v1(void);
static unsigned long long irq_receipt[8];
static unsigned long long catchups;

void jnox_original_rx_irq_stats_v1(unsigned long long out[8])
{
    if (bound_rq)
        irq_receipt[6] = bound_rq->calls;
    memcpy(out, irq_receipt, sizeof(irq_receipt));
}

unsigned long long jnox_rx_catchups_v1(void)
{
    return catchups;
}

static int schedule_rx(void)
{
    struct virtqueue *vq = bound_rq->vq;
    if (jnox_rx_napi_pending_v1()) {
        ++irq_receipt[7];
        return 0;
    }
    register unsigned long op asm("x8") = 174;
    register long result asm("x0") = bound_epoch;
    register unsigned long sequence asm("x1") = irq_receipt[5];
    register unsigned long reserved asm("x2") = 0;
    /* SVC174 wakes on queue-0 used progress. A shared virtio IRQ caused
     * solely by queue 1 must be handled by EL1 without publishing an RX event. */
    ++irq_receipt[4];
    asm volatile("svc #0x4a01" : "+r"(result), "+r"(sequence), "+r"(reserved)
                 : "r"(op) : "memory", "cc");
    if (result <= 0)
        return result < 0 ? result : -EPROTO;
    if (result == LONG_MAX) {
        /* EL1 found real used work across the wait boundary. Invoke the
         * registered queue callback but record the source independently. */
        ++catchups;
    } else {
        if ((unsigned long)result <= irq_receipt[5])
            return -EPROTO;
        irq_receipt[5] = result;
        ++irq_receipt[3];
    }
    vq->callback(vq);
    return jnox_rx_napi_pending_v1() ? 0 : -EPROTO;
}
#endif

#ifdef JNOX_PACKET_LIFECYCLE_ABI
int jnox_lifecycle_bind_worker_v1(struct virtio_device *vdev)
{
    struct virtnet_info *vi = vdev->priv;
    struct virtqueue *rx = jnox_packet_active_vq(0);
    if (!vi || !vi->rq || !rx || !rx->callback || vi->rq->vq != rx ||
        rx->vdev != vdev || vi->rq->napi.poll != virtnet_poll ||
        rx->index || !jnox_rx_equal_name_v1(rx->name, "input.0"))
        return -EPROTO;
    memset(receipt, 0, sizeof(receipt));
    memset(irq_receipt, 0, sizeof(irq_receipt));
    catchups = 0;
    bound_epoch = jnox_packet_token();
    bound_rq = vi->rq;
    receipt[0] = (unsigned long)bound_rq->napi.poll;
    receipt[1] = POLL_BUDGET;
    irq_receipt[0] = (unsigned long)rx->callback;
    irq_receipt[1] = rx->index;
    for (unsigned i = 0; rx->name[i]; ++i)
        irq_receipt[2] |= (unsigned long long)(unsigned char)rx->name[i] << (8*i);
    return 0;
}

void jnox_lifecycle_unbind_worker_v1(void)
{
    if (bound_rq) irq_receipt[6] = bound_rq->calls;
    bound_rq = NULL;
    bound_epoch = 0;
}
#else
static int read_mac(void)
{
    for (unsigned int i = 0; i < ETH_ALEN; ++i) {
        register unsigned long op asm("x8") = 158;
        register long value asm("x0") = i;
        register unsigned long width asm("x1") = 1;
        asm volatile("svc #0x4a01" : "+r"(value), "+r"(width) : "r"(op) : "memory", "cc");
        if (value < 0 || value > 255)
            return value < 0 ? value : -EPROTO;
        mac[i] = value;
    }
    return is_valid_ether_addr(mac) ? 0 : -EINVAL;
}

static int bind_poll(void)
{
    struct virtio_device *vdev = jnox_net_validated_device();
    struct virtqueue *rx = jnox_packet_active_vq(0);
    struct virtqueue *tx = jnox_packet_active_vq(1);
    unsigned long epoch = jnox_packet_token();
#ifdef JNOX_PACKET_RX_IRQ_ABI
    if (!vdev || rx || tx || epoch) return -ENODEV;
#else
    if (!vdev || !rx || !tx || !epoch || rx->index != 0 || tx->index != 1 ||
        rx->vdev != vdev || tx->vdev != vdev || rx->num_free != RX_DEPTH)
        return -ENODEV;
#endif
    if (bound_epoch || vdev->priv)
        return -EBUSY;
    int err = read_mac();
    if (err)
        return err;
    struct net_device *dev = &rx_netdev.alignment;
    dev->dev_addr = mac;
    dev->type = ARPHRD_ETHER;
    dev->hard_header_len = ETH_HLEN;
    dev->addr_len = ETH_ALEN;
    dev->mtu = ETH_DATA_LEN;
    dev->max_mtu = ETH_DATA_LEN;
    dev->priv_len = sizeof(struct virtnet_info);
    dev->num_tx_queues = dev->real_num_tx_queues = 1;
    dev->_tx = &txq;
    txq.dev = dev;
    struct virtnet_info *vi = netdev_priv(dev);
    vi->vdev = vdev;
    vi->dev = dev;
    vi->max_queue_pairs = vi->curr_queue_pairs = 1;
    vi->hdr_len = sizeof(struct virtio_net_hdr_v1);
    vdev->priv = vi;
    bound_epoch = epoch;
    err = jnox_rx_napi_prepare_v1(dev);
    if (err)
        return err;
    err = virtnet_alloc_queues(vi);
    if (err)
        return err;
    if (!vi->sq || !vi->rq || vi->rq->napi.dev != dev ||
        vi->rq->napi.poll != virtnet_poll || vi->rq->napi.weight < POLL_BUDGET)
        return -EPROTO;
#ifdef JNOX_PACKET_RX_IRQ_ABI
    err = virtnet_find_vqs(vi);
    if (err) return err;
    rx = jnox_packet_active_vq(0);
    tx = jnox_packet_active_vq(1);
    bound_epoch = jnox_packet_token();
    if (!rx || !tx || !bound_epoch || vi->rq->vq != rx || vi->sq->vq != tx ||
        rx->index || rx->num_free != RX_DEPTH || !rx->callback ||
        rx->name != vi->rq->name || !jnox_rx_equal_name_v1(rx->name, "input.0"))
        return -EPROTO;
    irq_receipt[0] = (unsigned long)rx->callback;
    irq_receipt[1] = rx->index;
    for (unsigned int i = 0; i < sizeof("input.0") - 1; ++i)
        irq_receipt[2] |= (unsigned long long)(unsigned char)rx->name[i] << (8*i);
    napi_enable(&vi->rq->napi);
#else
    vi->sq->vq = tx;
    vi->rq->vq = rx;
#endif
    vi->sq->napi.weight = 0; /* RX-only profile has no outstanding TX. */
    receipt[0] = (unsigned long)vi->rq->napi.poll;
    receipt[1] = POLL_BUDGET;
    for (unsigned int n = 0; n < RX_DEPTH; ++n) {
        err = add_recvbuf_small(vi, vi->rq, GFP_KERNEL);
        if (err)
            return err;
    }
    if (rx->num_free)
        return -EPROTO;
    bound_rq = vi->rq;
    return 0;
}

int jnox_original_rx_init(void)
{
    int ret = jnox_packet_context_enter_v1(0);
    if (!ret) {
        ret = bind_poll();
        jnox_packet_context_leave_v1();
    }
    return ret;
}

#endif

static int poll_read(void *out, unsigned int cap)
{
#ifdef JNOX_PACKET_LIFECYCLE_ABI
    extern bool jnox_lifecycle_is_up_v1(void);
    if (!jnox_lifecycle_is_up_v1()) return -ENETDOWN;
#endif
    if (!out || !cap)
        return -EINVAL;
    if (!bound_rq || bound_epoch != jnox_packet_token() ||
        bound_rq->vq != jnox_packet_active_vq(0))
        return -ENODEV;
    int len = jnox_rx_skb_read_v1(out, cap);
    if (len == -EAGAIN) {
#ifdef JNOX_PACKET_RX_IRQ_ABI
        int err = schedule_rx();
        if (err) return err;
#endif
        int work = jnox_rx_napi_poll_v1(&bound_rq->napi, POLL_BUDGET);
        ++receipt[2];
        receipt[6] = (long long)work;
        if (work < 0 || work > POLL_BUDGET)
            return work < 0 ? work : -EPROTO;
        receipt[3] += work;
        receipt[4] += work == POLL_BUDGET;
        receipt[7] += work > 0;
        unsigned long long stats[8];
        jnox_rx_vq_stats(stats);
        if ((long long)stats[7] < 0)
            return (int)(long long)stats[7];
        len = jnox_rx_skb_read_v1(out, cap);
    }
    if (len > 0)
        ++receipt[5];
    return len;
}

int jnox_original_rx_read(void *out, unsigned int capacity)
{
    int ret = jnox_packet_context_enter_v1(0);
    if (!ret) {
        ret = poll_read(out, capacity);
        jnox_packet_context_leave_v1();
    }
    return ret;
}

void jnox_original_rx_poll_stats_v1(unsigned long long out[8])
{
    memcpy(out, receipt, sizeof(receipt));
}
