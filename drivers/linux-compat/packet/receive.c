/* SPDX-License-Identifier: GPL-2.0-or-later */
/* First receive stage: original small-buffer provisioning and DMA unmap,
 * followed by a bounded native Ethernet consumer on the service FIFO. */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <jnox_tx_virtqueue.h>
#include <jnox_original_rx.h>
#include <jnox_rx_page_api.h>

extern int add_recvbuf_small(struct virtnet_info *, struct receive_queue *, gfp_t);
extern void *virtnet_rq_get_buf(struct receive_queue *, u32 *, void **);
extern void jnox_rx_vq_stats(unsigned long long out[8]);
extern unsigned long jnox_rx_span_v1(const void *, unsigned int);
extern struct virtio_device *jnox_net_validated_device(void);

enum { RX_DEPTH = 32, RX_HEADER = sizeof(struct virtio_net_hdr_v1) };
static_assert(RX_HEADER == 12);
static union {
    struct net_device alignment;
    unsigned char storage[offsetof(struct net_device, priv) + sizeof(struct virtnet_info)];
} rx_netdev;
static struct receive_queue rq;
static struct virtnet_info *owner;
static unsigned long epoch;
static bool running;

static int bind_rx(void)
{
    struct virtqueue *vq = jnox_packet_active_vq(0);
    struct virtio_device *vdev = jnox_net_validated_device();
    unsigned long token = jnox_packet_token();
    struct virtnet_info *vi;
    if (!token || !vdev || !vq || vq->index || vq->vdev != vdev)
        return -ENODEV;
    if (epoch || vdev->priv)
        return -EBUSY;
    if (vq->num_free != RX_DEPTH)
        return -EINVAL;

    struct net_device *dev = &rx_netdev.alignment;
    dev->priv_len = sizeof(struct virtnet_info);
    dev->mtu = ETH_DATA_LEN;
    dev->hard_header_len = ETH_HLEN;
    dev->addr_len = ETH_ALEN;
    vi = netdev_priv(dev);
    vi->dev = dev;
    vi->vdev = vdev;
    vi->max_queue_pairs = vi->curr_queue_pairs = 1;
    vi->hdr_len = RX_HEADER;
    vdev->priv = vi;
    epoch = token;
    owner = vi;
    rq.vq = vq;
    vi->rq = &rq;
    for (unsigned int n = 0; n < RX_DEPTH; ++n) {
        int err = add_recvbuf_small(vi, &rq, GFP_KERNEL);
        if (err)
            return err; /* Partial queues stay attached until owner-exit reset. */
    }
    if (vq->num_free)
        return -EPROTO;
    running = true;
    return 0;
}

int jnox_original_rx_init(void)
{
    int err = jnox_packet_context_enter_v1(0);
    if (err)
        return err;
    err = bind_rx();
    jnox_packet_context_leave_v1();
    return err;
}

static int read_rx(void *out, unsigned int cap)
{
    if (!out || !cap)
        return -EINVAL;
    if (!running || epoch != jnox_packet_token() || rq.vq != jnox_packet_active_vq(0))
        return -ENODEV;
    u32 len = 0;
    void *ctx = NULL;
    unsigned char *buf = virtnet_rq_get_buf(&rq, &len, &ctx);
    if (!buf) {
        unsigned long long stats[8];
        jnox_rx_vq_stats(stats);
        if ((long long)stats[7] < 0) {
            running = false;
            return (int)(long long)stats[7];
        }
        return -EAGAIN;
    }
    /* The adapter checked the returned descriptor against its posted span
     * before restoring this cookie. The skb's fragment reference is ours. */
    if (ctx || !jnox_rx_span_v1(buf, 1)) {
        running = false;
        return -EPROTO;
    }
    int result;
    if (len < RX_HEADER + ETH_HLEN || len > RX_HEADER + ETH_FRAME_LEN ||
        !jnox_rx_span_v1(buf, len) || buf[0] || buf[1]) {
        result = -EBADMSG;
    } else if (len - RX_HEADER > cap) {
        result = -EMSGSIZE;
    } else {
        result = len - RX_HEADER;
        memcpy(out, buf + RX_HEADER, result);
    }
    put_page(virt_to_head_page(buf));
    int err = add_recvbuf_small(owner, &rq, GFP_KERNEL);
    if (err || rq.vq->num_free) {
        running = false;
        return err ? err : -EPROTO;
    }
    return result;
}

int jnox_original_rx_read(void *out, unsigned int capacity)
{
    int ret = jnox_packet_context_enter_v1(0);
    if (ret)
        return ret;
    ret = read_rx(out, capacity);
    jnox_packet_context_leave_v1();
    return ret;
}
