// SPDX-License-Identifier: GPL-2.0-or-later
/* Original Linux small-RX pages are EL0 storage. The mapped address is a
 * bounded page-pool handle; EL1 grants one copy-back window per descriptor. */
#ifndef JNOX_PACKET_RX_ABI
#error "Requires the original receive-buffer transport"
#endif
#include <linux/dma-mapping.h>
#include <linux/errno.h>
#include <linux/scatterlist.h>
#include <linux/virtio.h>
#include <jnox_tx_virtqueue.h>
#include <jnox_rx_virtqueue.h>
#include <jnox_rx_page_api.h>

enum { QUEUE_SIZE = 32, MAP_COUNT = QUEUE_SIZE + 2, RX_CAPACITY = 12 + 14 + 4 + 1500 };
struct rx_map {
    unsigned long token;
    dma_addr_t base;
    unsigned int length;
    unsigned int posted;
};
struct rx_slot {
    unsigned long token;
    void *cookie, *context, *window;
    dma_addr_t handle;
    unsigned int length;
    struct rx_map *map;
};
static struct rx_map maps[MAP_COUNT];
static struct rx_slot slots[QUEUE_SIZE];
static unsigned long running;
static unsigned long long stats[8];

static long call_rx(unsigned long op, unsigned long epoch, const void *ptr, unsigned long len)
{
    register unsigned long x0 asm("x0") = epoch;
    register unsigned long x1 asm("x1") = (unsigned long)ptr;
    register unsigned long x2 asm("x2") = len;
    register unsigned long x8 asm("x8") = op;
    asm volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static long fault(long error)
{
    if (!stats[7]) stats[7] = error;
    return error;
}

static bool active(const struct virtqueue *vq)
{
    return vq && vq == jnox_packet_active_vq(0) && jnox_packet_token();
}

static struct rx_map *range(dma_addr_t addr, size_t len)
{
    unsigned long epoch = jnox_packet_token();
    for (unsigned i = 0; i < MAP_COUNT; ++i) {
        struct rx_map *m = &maps[i];
        if (epoch && m->token == epoch && addr >= m->base && len <= m->length &&
            addr - m->base <= m->length - len) return m;
    }
    return NULL;
}

dma_addr_t virtqueue_map_single_attrs(const struct virtqueue *vq, void *ptr, size_t size,
                                      enum dma_data_direction dir, unsigned long attrs)
{
    if (!active(vq) || dir != DMA_FROM_DEVICE || attrs || !size || size > PAGE_SIZE)
        return DMA_MAPPING_ERROR;
    unsigned long base = jnox_rx_span_v1(ptr, size);
    if (!base || jnox_rx_resolve_v1(base, size) != ptr) return DMA_MAPPING_ERROR;
    struct rx_map *free = NULL;
    for (unsigned i = 0; i < MAP_COUNT; ++i) {
        struct rx_map *m = &maps[i];
        if (!m->token) {
            if (!free) free = m;
        } else if (base >= m->base ? base - m->base < m->length : m->base - base < size) {
            return DMA_MAPPING_ERROR;
        }
    }
    if (!free) return DMA_MAPPING_ERROR;
    *free = (struct rx_map){ .token = jnox_packet_token(), .base = base, .length = size };
    return base;
}

int virtqueue_map_mapping_error(const struct virtqueue *vq, dma_addr_t addr)
{
    return !active(vq) || addr == DMA_MAPPING_ERROR || !range(addr, 1);
}

void virtqueue_unmap_single_attrs(const struct virtqueue *vq, dma_addr_t addr,
                                  size_t size, enum dma_data_direction dir, unsigned long attrs)
{
    struct rx_map *m = range(addr, size);
    if (!active(vq) || !m || addr != m->base || size != m->length || m->posted ||
        dir != DMA_FROM_DEVICE || (attrs & ~DMA_ATTR_SKIP_CPU_SYNC)) {
        fault(-EPROTO);
        return;
    }
    *m = (struct rx_map){};
}

bool virtqueue_map_need_sync(const struct virtqueue *vq, dma_addr_t addr)
{
    if (!active(vq) || !range(addr, 1)) fault(-EINVAL);
    /* EL1 copies completed DMA bytes before SVC172 returns. The EL0 page
     * is never a device DMA target and requires no separate cache operation. */
    return false;
}

static void sync_range(const struct virtqueue *vq, dma_addr_t addr, unsigned long offset,
                       size_t size, enum dma_data_direction dir)
{
    struct rx_map *m = range(addr, 1);
    if (!active(vq) || !m || addr != m->base || dir != DMA_FROM_DEVICE ||
        size > m->length || offset > m->length - size) fault(-EINVAL);
}

void virtqueue_map_sync_single_range_for_cpu(const struct virtqueue *vq, dma_addr_t addr,
                                             unsigned long offset, size_t size, enum dma_data_direction dir)
{
    sync_range(vq, addr, offset, size, dir);
}
void virtqueue_map_sync_single_range_for_device(const struct virtqueue *vq, dma_addr_t addr,
                                                unsigned long offset, size_t size, enum dma_data_direction dir)
{
    sync_range(vq, addr, offset, size, dir);
}

int virtqueue_add_inbuf_premapped(struct virtqueue *vq, struct scatterlist *sg, unsigned num,
                                  void *cookie, void *ctx, gfp_t gfp)
{
    (void)gfp;
    if (!active(vq) || num != 1 || !sg || sg_is_chain(sg) || !sg_is_last(sg) || !cookie)
        return -EINVAL;
    if (!vq->num_free) return -ENOSPC;
    unsigned len = sg_dma_len(sg);
    dma_addr_t addr = sg_dma_address(sg);
    struct rx_map *m = range(addr, len);
    if (len != RX_CAPACITY || !m) return -EINVAL;
    void *window = jnox_rx_resolve_v1(addr, len);
    /* For the admitted add_recvbuf_small layout, data is exactly the posted
     * header address. Keep data and ctx unchanged for virtqueue_get_buf_ctx. */
    if (!window || window != cookie || jnox_rx_span_v1(window, len) != addr) return -EFAULT;
    unsigned long epoch = jnox_packet_token();
    long id = call_rx(171, epoch, window, len);
    if (id < 0) return id;
    if (id >= QUEUE_SIZE || slots[id].token) {
        call_rx(1, 96, NULL, 0);
        __builtin_unreachable();
    }
    slots[id] = (struct rx_slot){.token = epoch, .cookie = cookie, .context = ctx,
                                  .window = window, .handle = addr, .length = len, .map = m};
    ++m->posted;
    --vq->num_free;
    ++stats[0];
    ++stats[2];
    stats[3] = id;
    return 0;
}

long jnox_rx_start_v1(void)
{
    struct virtqueue *vq = jnox_packet_active_vq(0);
    unsigned long epoch = jnox_packet_token();
    if (!active(vq)) return -ENODEV;
    if (running) return -EBUSY;
    if (vq->num_free || stats[2] != QUEUE_SIZE) return -EAGAIN;
    long rc = call_rx(170, epoch, NULL, 0);
    if (rc) return rc;
    running = epoch;
    ++stats[6];
    return 0;
}

void *virtqueue_get_buf_ctx(struct virtqueue *vq, unsigned *len, void **ctx)
{
    unsigned long epoch = jnox_packet_token();
    if (!active(vq) || running != epoch || !len) {
        fault(-ENODEV);
        return NULL;
    }
    long result = call_rx(172, epoch, NULL, 0);
    if (result == -EAGAIN) return NULL;
    if (result < 0) {
        fault(result);
        return NULL;
    }
    unsigned int id = (unsigned long)result & 0xffff;
    unsigned long bytes = (unsigned long)result >> 16;
    if (id >= QUEUE_SIZE) goto invalid;
    struct rx_slot *s = &slots[id];
    if (s->token != epoch || !s->map || s->map->token != epoch || !s->map->posted ||
        bytes < 12 + 14 || bytes > s->length || vq->num_free >= QUEUE_SIZE ||
        jnox_rx_resolve_v1(s->handle, s->length) != s->window) goto invalid;
    void *cookie = s->cookie;
    if (ctx) *ctx = s->context;
    *len = bytes;
    --s->map->posted;
    *s = (struct rx_slot){};
    ++vq->num_free;
    ++stats[1];
    --stats[2];
    stats[4] = id;
    stats[5] = bytes;
    return cookie;
invalid:
    fault(-EPROTO);
    /* EL1 has consumed this entry. Its ownership cannot be reconstructed. */
    call_rx(1, 97, NULL, 0);
    __builtin_unreachable();
}

void jnox_rx_reset_complete_v1(unsigned long retired_token)
{
    /* The queue owner calls here only after a confirmed transport reset. */
    if (!retired_token) return;
    for (unsigned i = 0; i < QUEUE_SIZE; ++i) {
        if (slots[i].token != retired_token) continue;
        slots[i] = (struct rx_slot){};
        --stats[2];
    }
    for (unsigned i = 0; i < MAP_COUNT; ++i)
        if (maps[i].token == retired_token) maps[i] = (struct rx_map){};
    if (running == retired_token) running = 0;
}

unsigned long jnox_rx_posted_address_v1(unsigned int id)
{
    unsigned long epoch = jnox_packet_token();
    if (!epoch || id >= QUEUE_SIZE || slots[id].token != epoch) return 0;
    return (unsigned long)slots[id].window;
}

void jnox_rx_vq_stats(unsigned long long out[8])
{
    for (unsigned i = 0; i < 8; ++i) out[i] = stats[i];
}
