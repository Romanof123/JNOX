// SPDX-License-Identifier: GPL-2.0-or-later
/* A linear skb may contribute one contiguous segment or a 12-byte control
 * header and one payload. These CPU-side scatterlists never contain DMA
 * addresses. The kernel copies each validated submission to one descriptor. */
#include <linux/errno.h>
#include <linux/mm_types.h>
#include <linux/scatterlist.h>
#include <linux/skbuff.h>
#include <linux/string.h>
#include <linux/virtio.h>
#include <jnox_tx_virtqueue.h>
#ifdef JNOX_PACKET_RX_ABI
#include <jnox_rx_virtqueue.h>
#endif

enum { QSIZE = 32, HEADER = 12, MIN_FRAME = 14, MAX_FRAME = 1514 };

struct tx_storage {
    struct sk_buff *skb;
    const unsigned char *head, *payload;
    unsigned int capacity, payload_len;
    unsigned long epoch;
    bool outstanding;
    /* Local SG page identities for the bounded backing and control area. */
    struct page linear_page, control_page;
};

struct tx_cookie {
    struct tx_storage *storage;
    void *cookie;
    unsigned long epoch;
};

static struct tx_storage storage[QSIZE];
static struct tx_cookie pending[QSIZE];
static struct scatterlist *table;
static unsigned int nents;
static unsigned long notify_epoch;
static bool need_notify;
static unsigned char wire[HEADER + MAX_FRAME];
static unsigned long long stats[8];
volatile unsigned long long jnox_packet_sg_counts[2];

static long svc(unsigned long op, unsigned long epoch, const void *buffer, unsigned int size)
{
    register unsigned long x0 asm("x0") = epoch;
    register unsigned long x1 asm("x1") = (unsigned long)buffer;
    register unsigned long x2 asm("x2") = size;
    register unsigned long x8 asm("x8") = op;
    asm volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static bool within(const void *p, unsigned int len, const void *base, unsigned int capacity)
{
    unsigned long addr = (unsigned long)p, start = (unsigned long)base;
    return addr >= start && len <= capacity && addr - start <= capacity - len;
}

static struct tx_storage *lookup(const struct sk_buff *skb)
{
    for (unsigned int i = 0; i < QSIZE; ++i)
        if (skb && storage[i].skb == skb) return &storage[i];
    return NULL;
}

int jnox_tx_register_skb(struct sk_buff *skb, const void *base, unsigned int capacity)
{
    unsigned long epoch = jnox_packet_token();
    if (!epoch || !skb || ((unsigned long)skb & 3) || !base || skb->head != base ||
        !capacity || capacity > PAGE_SIZE || skb->data_len ||
        skb->len < MIN_FRAME || skb->len > MAX_FRAME ||
        !within(skb->data, skb->len, base, capacity)) return -EINVAL;
    if (lookup(skb)) return -EBUSY;
    for (unsigned int i = 0; i < QSIZE; ++i) {
        struct tx_storage *s = &storage[i];
        if (s->skb) continue;
        s->skb = skb;
        s->head = base;
        s->capacity = capacity;
        s->payload = skb->data;
        s->payload_len = skb->len;
        s->epoch = epoch;
        s->outstanding = false;
        return 0;
    }
    return -ENOSPC;
}

int jnox_tx_unregister_skb(struct sk_buff *skb)
{
    struct tx_storage *s = lookup(skb);
    if (!s) return -ENOENT;
    if (s->outstanding) return -EBUSY;
    s->skb = NULL;
    return 0;
}

void sg_init_table(struct scatterlist *sg, unsigned int count)
{
    table = NULL;
    nents = 0;
    if (!sg || count == 0 || count > 2) return;
    memset(sg, 0, count * sizeof(*sg));
    sg_init_marker(sg, count);
    table = sg;
    nents = count;
}

static bool in_table(const struct scatterlist *sg)
{
    return table && (sg == table || (nents == 2 && sg == table + 1));
}

static void assign(struct scatterlist *sg, struct page *page, unsigned int offset, unsigned int len)
{
    /* These are local page identities with offsets bounded to one page. */
    sg_assign_page(sg, page);
    sg->offset = offset;
    sg->length = len;
}

void jnox_sg_set_buf_v1(struct scatterlist *sg, const void *buf, unsigned int len)
{
    if (!in_table(sg)) return;
    /* The separate header produced by xmit_skb starts at skb->cb. */
    for (unsigned int i = 0; i < QSIZE; ++i) {
        struct tx_storage *s = &storage[i];
        if (s->skb && !s->outstanding && s->epoch == jnox_packet_token() &&
            sg == table && nents == 2 && buf == s->skb->cb && len == HEADER) {
            assign(sg, &s->control_page, 0, len);
            return;
        }
    }
    sg->length = 0;
}

int skb_to_sgvec(struct sk_buff *skb, struct scatterlist *sg, int offset, int len)
{
    struct tx_storage *s = lookup(skb);
    if (!s || s->outstanding || s->epoch != jnox_packet_token() || !in_table(sg) ||
        sg != &table[nents - 1] || offset < 0 || len <= 0 || skb->data_len ||
        (unsigned int)offset > skb->len || (unsigned int)len > skb->len - offset ||
        !within(skb->data, skb->len, s->head, s->capacity)) return -EINVAL;
    unsigned long off = (unsigned long)skb->data - (unsigned long)s->head + offset;
    assign(sg, &s->linear_page, off, len);
    sg_mark_end(sg);
    return 1;
}

static int stage(struct tx_storage *s, struct scatterlist sg[], unsigned int count)
{
    unsigned int bytes = 0;
    for (unsigned int i = 0; i < count; ++i) {
        const struct scatterlist *entry = &sg[i];
        const unsigned char *src;
        if (sg_is_chain(entry) || !!sg_is_last(entry) != (i + 1 == count) ||
            !entry->length || entry->length > sizeof(wire) - bytes) return -EINVAL;
        if (count == 2 && i == 0) {
            if (sg_page(&sg[i]) != &s->control_page || entry->offset || entry->length != HEADER)
                return -EINVAL;
            src = s->skb->cb;
        } else {
            unsigned int prefix = count == 1 ? HEADER : 0;
            if (sg_page(&sg[i]) != &s->linear_page || entry->offset > s->capacity ||
                entry->length > s->capacity - entry->offset ||
                entry->length != s->payload_len + prefix) return -EINVAL;
            src = s->head + entry->offset;
            if (src + prefix != s->payload) return -EINVAL;
        }
        memcpy(wire + bytes, src, entry->length);
        bytes += entry->length;
    }
    return bytes;
}

int virtqueue_add_outbuf(struct virtqueue *vq, struct scatterlist sg[], unsigned int num,
                         void *cookie, gfp_t gfp)
{
    (void)gfp;
    unsigned long epoch = jnox_packet_token();
    unsigned long tagged = (unsigned long)cookie;
    struct tx_storage *s = lookup((struct sk_buff *)(tagged & ~3UL));
    if (!epoch || !vq || vq != jnox_packet_active_vq(1) || !s || s->epoch != epoch ||
        (tagged & 3) > 1 || s->outstanding || sg != table || !num || num > 2 ||
        num != nents) return -EINVAL;
    if (!vq->num_free) return -ENOSPC;
    int bytes = stage(s, sg, num);
    if (bytes < 0) return bytes;
    long id = svc(164, epoch, wire, bytes);
    if (id < 0) {
        stats[7] = id;
        return id;
    }
    /* A duplicate id would strand the earlier cookie. Stop this service;
     * kernel owner-exit resets the transport before another claim. */
    if (id >= QSIZE || pending[id].cookie) {
        svc(1, 93, NULL, 0);
        __builtin_unreachable();
    }
    stats[7] = 0;
    pending[id] = (struct tx_cookie){.storage = s, .cookie = cookie, .epoch = epoch};
    s->outstanding = true;
    --vq->num_free;
    need_notify = true;
    notify_epoch = epoch;
    ++stats[0];
    ++jnox_packet_sg_counts[num - 1];
    stats[3] = id;
    stats[4] = num;
    stats[5] = bytes;
    ++stats[6];
    return 0;
}

/* SVC169 updates the real TX avail-ring flags and checks the used index on
 * rearm. A reported pending completion makes Linux schedule another poll. */
static long tx_callbacks(struct virtqueue *vq, unsigned int mode)
{
    unsigned long epoch = jnox_packet_token();
    long rc = -ENODEV;
    if (epoch && vq && vq == jnox_packet_active_vq(1))
        rc = svc(169, epoch, (const void *)(unsigned long)mode, 0);
    if (rc < 0 || rc > (long)mode) {
        stats[7] = rc < 0 ? rc : -EPROTO;
        svc(1, 95, NULL, 0);
        __builtin_unreachable();
    }
    return rc;
}

void virtqueue_disable_cb(struct virtqueue *vq)
{
    tx_callbacks(vq, 0);
}

bool virtqueue_enable_cb_delayed(struct virtqueue *vq)
{
    return tx_callbacks(vq, 1) == 1;
}

bool virtqueue_kick_prepare(struct virtqueue *vq)
{
    return vq && vq == jnox_packet_active_vq(1) && need_notify && notify_epoch == jnox_packet_token();
}

bool virtqueue_notify(struct virtqueue *vq)
{
    if (!vq || vq != jnox_packet_active_vq(1) || !jnox_packet_token()) return false;
    long rc = svc(165, jnox_packet_token(), NULL, 0);
    if (rc) {
        stats[7] = rc;
        return false;
    }
    stats[7] = 0;
    need_notify = false;
    ++stats[1];
    return true;
}

void *virtqueue_get_buf(struct virtqueue *vq, unsigned int *len)
{
#ifdef JNOX_PACKET_RX_ABI
    if (vq && vq == jnox_packet_active_vq(0))
        return virtqueue_get_buf_ctx(vq, len, NULL);
#endif
    unsigned long epoch = jnox_packet_token();
    if (!epoch || !vq || vq != jnox_packet_active_vq(1) || !len) return NULL;
    long id = svc(166, epoch, NULL, 0);
    if (id == -EAGAIN) return NULL;
    if (id < 0) {
        stats[7] = id;
        return NULL;
    }
    if (id >= QSIZE || !pending[id].cookie || pending[id].epoch != epoch ||
        !pending[id].storage->outstanding || vq->num_free >= QSIZE) {
        svc(1, 94, NULL, 0);
        __builtin_unreachable();
    }
    stats[7] = 0;
    void *cookie = pending[id].cookie;
    pending[id].storage->outstanding = false;
    pending[id] = (struct tx_cookie){};
    ++vq->num_free;
    ++stats[2];
    --stats[6];
    *len = 0; /* The TX descriptor is entirely device-readable. */
    return cookie;
}

/* The owner calls this only after EL1 has confirmed the device reset. A
 * canceled cookie becomes reclaimable, but is not counted as a used entry. */
void jnox_tx_reset_complete(unsigned long epoch)
{
#ifdef JNOX_PACKET_RX_ABI
    jnox_rx_reset_complete_v1(epoch);
#endif
    for (unsigned int i = 0; i < QSIZE; ++i) {
        if (pending[i].cookie && pending[i].epoch == epoch) {
            pending[i].storage->outstanding = false;
            pending[i] = (struct tx_cookie){};
            --stats[6];
        }
    }
    if (notify_epoch == epoch) {
        notify_epoch = 0;
        need_notify = false;
    }
    table = NULL;
    nents = 0;
}

void jnox_tx_vq_stats(unsigned long long out[8])
{
    for (unsigned int i = 0; i < 8; ++i) out[i] = stats[i];
}
