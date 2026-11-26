/* SPDX-License-Identifier: GPL-2.0-or-later */
/* This TX-only fixture constructs the state needed by the pinned start_xmit.
 * Full probe/open registration is a separate stage. All Linux private types
 * below come from the builder's verified, declaration-only upstream slices.
 */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <jnox_original_tx.h>
#include <jnox_tx_virtqueue.h>
#include <jnox_packet_tx_runtime.h>
#include <linux/if_arp.h>

extern netdev_tx_t start_xmit(struct sk_buff *, struct net_device *);
extern struct virtio_device *jnox_net_validated_device(void);

enum {
    TX_MAX_FRAME = ETH_HLEN + ETH_DATA_LEN,
    TX_HEADROOM = 64,
    TX_LINEAR = SKB_DATA_ALIGN(TX_HEADROOM + TX_MAX_FRAME),
    TX_HEADER = sizeof(struct virtio_net_hdr_v1),
};
static_assert(TX_HEADER == 12);

/* net_device::priv is a cacheline-aligned flexible array in this Linux tree.
 * The allocation includes all of virtnet_info, including its trailing union.
 */
static union {
    struct net_device alignment;
    unsigned char storage[offsetof(struct net_device, priv) + sizeof(struct virtnet_info)];
} netdev_memory;
static struct send_queue send_queue;
static struct netdev_queue netdev_queue;
static struct sk_buff frame_skb;
static unsigned char frame_memory[TX_LINEAR + SKB_DATA_ALIGN(sizeof(struct skb_shared_info))]
    __aligned(SMP_CACHE_BYTES);
static unsigned long bound_epoch;
static unsigned int frame_sequence;
static enum { TX_IDLE, TX_STAGED, TX_POSTED, TX_USED, TX_DROPPED, TX_FAILED } state;

static struct net_device *tx_netdev(void)
{
    return &netdev_memory.alignment;
}

static int bind_tx(void)
{
    struct virtqueue *vq = jnox_packet_active_vq(1);
    struct virtio_device *vdev = jnox_net_validated_device();
    struct net_device *dev = tx_netdev();
    struct virtnet_info *vi;
    unsigned long epoch = jnox_packet_token();

    if (!epoch || !vq || !vdev || vq->vdev != vdev || vq->index != 1)
        return -ENODEV;
    if (state != TX_IDLE || bound_epoch == epoch)
        return -EBUSY;
    if (vdev->priv && vdev->priv != (void *)dev->priv)
        return -EBUSY;
    if (vq->num_free < MAX_SKB_FRAGS + 4)
        return -ENOSPC;
    memset(&netdev_memory, 0, sizeof(netdev_memory));
    memset(&send_queue, 0, sizeof(send_queue));
    memset(&netdev_queue, 0, sizeof(netdev_queue));

    dev->priv_len = sizeof(struct virtnet_info);
    vi = netdev_priv(dev);
    dev->mtu = ETH_DATA_LEN;
    dev->hard_header_len = ETH_HLEN;
    dev->type = ARPHRD_ETHER;
    dev->addr_len = ETH_ALEN;
    dev->num_tx_queues = dev->real_num_tx_queues = 1;
    dev->_tx = &netdev_queue;
    netdev_queue.dev = dev;
    netdev_queue.xmit_lock_owner = -1;
    spin_lock_init(&netdev_queue._xmit_lock);
#ifdef CONFIG_BQL
    netdev_queue.dql.min_limit = TX_MAX_FRAME;
    netdev_queue.dql.max_limit = TX_MAX_FRAME;
    netdev_queue.dql.limit = TX_MAX_FRAME;
    netdev_queue.dql.adj_limit = TX_MAX_FRAME;
#endif
    send_queue.vq = vq;
    send_queue.napi.weight = 0;
    u64_stats_init(&send_queue.stats.syncp);
    vi->vdev = vdev;
    vi->dev = dev;
    vi->sq = &send_queue;
    vi->max_queue_pairs = vi->curr_queue_pairs = 1;
    vi->hdr_len = TX_HEADER;
    vi->any_header_sg = true;
    virtqueue_disable_cb(vq);
    if (!virtqueue_enable_cb_delayed(vq))
        return -EIO;
    vdev->priv = vi;
    bound_epoch = epoch;
    return 0;
}

static int enter_tx(void)
{
    int ret = jnox_packet_context_enter_v1(0);
    if (!ret) {
        ret = jnox_packet_tx_clock_v1();
        if (ret)
            jnox_packet_context_leave_v1();
    }
    return ret;
}

int jnox_original_tx_init(void)
{
    int ret = enter_tx();
    if (ret)
        return ret;
    ret = bind_tx();
    jnox_packet_context_leave_v1();
    return ret;
}

static bool bound(void)
{
    return bound_epoch && jnox_packet_token() == bound_epoch &&
        send_queue.vq == jnox_packet_active_vq(1);
}

static void make_skb(const unsigned char *frame, unsigned int len, bool push)
{
    struct sk_buff *skb = &frame_skb;
    memset(skb, 0, sizeof(*skb));
    memset(frame_memory, 0, sizeof(frame_memory));
    skb->head = frame_memory;
    skb->data = frame_memory + (push ? TX_HEADROOM : 0);
    skb_set_end_offset(skb, TX_LINEAR);
    skb_set_tail_pointer(skb, len);
    skb->len = len;
    skb->truesize = SKB_TRUESIZE(TX_LINEAR);
    skb->dev = tx_netdev();
    skb->ip_summed = CHECKSUM_NONE;
    skb->protocol = cpu_to_be16(((u16)frame[12] << 8) | frame[13]);
    skb_reset_mac_header(skb);
    skb->mac_len = ETH_HLEN;
    skb_set_network_header(skb, ETH_HLEN);
    skb->transport_header = (typeof(skb->transport_header))~0U;
    skb_set_queue_mapping(skb, 0);
    refcount_set(&skb->users, 1);
    atomic_set(&skb_shinfo(skb)->dataref, 1);
    memcpy(skb->data, frame, len);
}

int jnox_packet_retire_skb_v1(struct sk_buff *skb, unsigned int reason)
{
    bool used = reason == SKB_CONSUMED;
    if (!bound() || skb != &frame_skb ||
        (used ? state != TX_POSTED : state != TX_STAGED))
        return -EINVAL;
    if (refcount_read(&skb->users) != 1 || atomic_read(&skb_shinfo(skb)->dataref) != 1)
        return -EPROTO;
    /* The transport refuses unregister while the device owns the descriptor.
     * A completion comes only from virtqueue_get_buf returning this cookie.
     */
    int ret = jnox_tx_unregister_skb(skb);
    if (ret)
        return ret;
    refcount_set(&skb->users, 0);
    atomic_set(&skb_shinfo(skb)->dataref, 0);
    state = used ? TX_USED : TX_DROPPED;
    return 0;
}

static int submit(const unsigned char *frame, unsigned int len, unsigned int sequence)
{
    unsigned long long before[8], after[8];
    bool push = !(sequence & 1U);
    int ret;
    if (!bound())
        return -ENODEV;
    if (!frame || len < ETH_HLEN || len > TX_MAX_FRAME)
        return -EINVAL;
    if (state != TX_IDLE)
        return -EBUSY;
    make_skb(frame, len, push);
    ret = jnox_tx_register_skb(&frame_skb, frame_memory, TX_LINEAR);
    if (ret)
        return ret;
    frame_sequence = sequence;
    state = TX_STAGED;
    jnox_tx_vq_stats(before);
    netdev_tx_t result = start_xmit(&frame_skb, tx_netdev());
    jnox_tx_vq_stats(after);
    if (state == TX_DROPPED) {
        state = TX_IDLE;
        return -EIO;
    }
    if (result != NETDEV_TX_OK || after[0] != before[0] + 1 ||
        after[1] != before[1] + 1 || after[4] != (push ? 1U : 2U) ||
        after[5] != len + TX_HEADER || after[6] != 1 || (long long)after[7] < 0) {
        /* A published descriptor may still be device-owned. Quarantine the
         * static backing; the owner must retire/reset it before service exit.
         */
        state = TX_FAILED;
        return -EPROTO;
    }
    state = TX_POSTED;
    return 0;
}

int jnox_original_tx_frame(const unsigned char *frame, unsigned int len, unsigned int sequence)
{
    int ret = enter_tx();
    if (ret)
        return ret;
    ret = submit(frame, len, sequence);
    jnox_packet_context_leave_v1();
    return ret;
}

static int reap(unsigned int sequence)
{
    unsigned int len = 0;
    void *cookie;
    if (!bound())
        return -ENODEV;
    if (state != TX_POSTED || frame_sequence != sequence)
        return -ENOENT;
    cookie = virtqueue_get_buf(send_queue.vq, &len);
    if (!cookie) {
        unsigned long long stats[8];
        jnox_tx_vq_stats(stats);
        return (long long)stats[7] < 0 ? (int)(long long)stats[7] : -EAGAIN;
    }
    if (len != 0 || (unsigned long)cookie !=
                    ((unsigned long)&frame_skb | VIRTNET_XMIT_TYPE_SKB_ORPHAN)) {
        state = TX_FAILED;
        return -EPROTO;
    }
    napi_consume_skb(&frame_skb, 0);
    if (state != TX_USED) {
        state = TX_FAILED;
        return -EPROTO;
    }
    state = TX_IDLE;
    return 0;
}

int jnox_original_tx_complete(unsigned int sequence)
{
    int ret = enter_tx();
    if (ret)
        return ret;
    ret = reap(sequence);
    jnox_packet_context_leave_v1();
    return ret;
}
