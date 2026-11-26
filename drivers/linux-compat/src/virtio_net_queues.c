// SPDX-License-Identifier: GPL-2.0-or-later
#include <linux/errno.h>
#include <linux/virtio.h>
#include <linux/virtio_config.h>

extern struct virtio_device *jnox_net_validated_device(void);
extern void jnox_net_forget_validation(void);

struct native_queue {
    struct virtqueue vq;
    bool ctx;
    u64 state;
};
static struct native_queue queues[2];
static unsigned long token;
static unsigned completions[2];

static long transport(unsigned long number, unsigned long arg, unsigned long index)
{
    register unsigned long x0 asm("x0") = arg;
    register unsigned long x1 asm("x1") = index;
    register unsigned long x8 asm("x8") = number;
    asm volatile("svc #0x4a01" : "+r"(x0), "+r"(x1) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

#ifdef JNOX_PACKET_TX_ABI
extern void jnox_tx_reset_complete(unsigned long released_token);
struct virtqueue *jnox_packet_active_vq(unsigned int index)
{
    return token && index < 2 ? &queues[index].vq : NULL;
}
unsigned long jnox_packet_token(void)
{
    return token;
}
#endif

void jnox_net_forget_queues(void);

static int abandon_pair(int cause)
{
    long release = transport(156, 0, 0);
#ifdef JNOX_PACKET_TX_ABI
    /* A failed reset quarantines the old buffers. Never report those skbs as
     * reclaimable merely because this userspace queue object was invalidated. */
    if (release < 0) token = 0;
#endif
    jnox_net_forget_queues();
    return release < 0 ? release : cause;
}

/* Only EL1 can allocate or address DMA pages. Publish both Linux queue
 * objects after the complete transport pair is live. */
noinline int jnox_queue_find_vqs(struct virtio_device *vdev, unsigned int count,
                                 struct virtqueue *vqs[], struct virtqueue_info info[],
                                 struct irq_affinity *affinity)
{
    if (!vdev || vdev != jnox_net_validated_device()) return -ENODEV;
    if (count != 2 || !vqs || !info || affinity ||
        !info[0].name || !*info[0].name || !info[1].name || !*info[1].name ||
        !info[0].callback || !info[1].callback || info[1].ctx) return -EINVAL;
    vqs[0] = vqs[1] = NULL;
    if (token) return -EBUSY;
    long handle = transport(161, 0, 0);
    if (handle <= 0)
        return handle < 0 ? handle : -EPROTO;
    for (unsigned i = 0; i < 2; ++i) {
        long state = transport(162, handle, i);
        if (state < 0) return abandon_pair(state);
        u32 max = (u64)state >> 32, size = ((u64)state >> 16) & 0xffff;
        if ((state & 0xffff) != 1 || size != 32 || max < size) return abandon_pair(-EPROTO);
        queues[i].state = state;
        queues[i].ctx = info[i].ctx;
        queues[i].vq = (struct virtqueue) {
            .vdev = vdev, .name = info[i].name, .callback = info[i].callback,
            .index = i, .num_free = size, .num_max = max,
        };
    }
    /* Fixed pair, one native owner, no concurrent list readers during setup. */
    struct list_head *head = &vdev->vqs;
    struct list_head *rx = &queues[0].vq.list, *tx = &queues[1].vq.list;
    head->next = rx;
    rx->prev = head;
    rx->next = tx;
    tx->prev = rx;
    tx->next = head;
    head->prev = tx;
    token = handle;
    vqs[0] = &queues[0].vq;
    vqs[1] = &queues[1].vq;
    return 0;
}

void jnox_net_forget_queues(void)
{
#ifdef JNOX_PACKET_TX_ABI
    if (token) jnox_tx_reset_complete(token);
#endif
    struct virtio_device *vdev = jnox_net_validated_device();
    if (vdev) INIT_LIST_HEAD(&vdev->vqs);
    for (unsigned i = 0; i < 2; ++i) {
        queues[i] = (struct native_queue){};
        INIT_LIST_HEAD(&queues[i].vq.list);
    }
    token = 0;
    jnox_net_forget_validation();
}

/* Fixture clients register distinct typed handlers. No descriptors are
 * posted in this profile, so observing a completion is a failure. */
static void rx_complete(struct virtqueue *vq)
{
    if (vq == &queues[0].vq) ++completions[0];
}

static void tx_complete(struct virtqueue *vq)
{
    if (vq == &queues[1].vq) ++completions[1];
}

long jnox_net_setup_queues(unsigned mode, unsigned long long *receipt)
{
    struct virtio_device *vdev = jnox_net_validated_device();
    struct virtqueue *pair[2] = {};
    struct virtqueue_info info[2] = {
        {.name = "input.0", .callback = rx_complete, .ctx = true},
        {.name = "output.0", .callback = tx_complete},
    };
    if (!vdev || !receipt || mode > 2) return -EINVAL;
    if (mode == 2) info[1].callback = NULL;
    int rc = virtio_find_vqs(vdev, mode == 1 ? 1 : 2, pair, info, NULL);
    if (rc) return rc;
    if (!pair[0] || !pair[1] || pair[0] == pair[1]) return abandon_pair(-EPROTO);
    receipt[0] = token;
    receipt[7] = 0;
    for (unsigned i = 0; i < 2; ++i) {
        struct virtqueue *vq = pair[i];
        receipt[1 + i] = queues[i].state;
        receipt[3 + i] = vq->num_free;
        receipt[5 + i] = vq->num_max;
        if (vq == &queues[i].vq && vq->vdev == vdev && vq->index == i &&
            vq->name == info[i].name && vq->callback == info[i].callback &&
            queues[i].ctx == info[i].ctx && vq->list.next->prev == &vq->list &&
            vq->list.prev->next == &vq->list && vdev->vqs.next == &pair[0]->list &&
            vdev->vqs.prev == &pair[1]->list)
            receipt[7] |= 1U << i;
        receipt[8 + i] = completions[i];
    }
    return 0;
}
