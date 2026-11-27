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
#ifdef JNOX_PACKET_LIFECYCLE_ABI
#include <uapi/linux/virtio_config.h>
#include "lifecycle_platform.h"
#endif
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
#ifdef JNOX_PACKET_RX_POLL_ABI
static bool need_rx_notify;
#endif
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
#ifdef JNOX_PACKET_RX_POLL_ABI
    if (running == epoch) need_rx_notify = true;
#endif
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

#ifdef JNOX_PACKET_LIFECYCLE_ABI
static unsigned long retired_epoch;

int jnox_rx_lifecycle_ready_v1(void)
{
    struct virtqueue *vq = jnox_packet_active_vq(0);
    unsigned long epoch = jnox_packet_token();
    if (!active(vq) || running || retired_epoch || stats[2] ||
        vq->num_free != QUEUE_SIZE) return -EINVAL;
    long rc = call_rx(177, epoch, NULL, 0);
    if (rc) return rc;
    running = epoch;
    ++stats[6];
    return 0;
}

void jnox_rx_lifecycle_retire_v1(unsigned long epoch)
{
    if (!epoch || epoch != jnox_packet_token() || retired_epoch)
        __builtin_trap();
    retired_epoch = epoch;
}

void *virtqueue_detach_unused_buf(struct virtqueue *vq)
{
    if (!retired_epoch || retired_epoch != jnox_packet_token()) __builtin_trap();
    if (vq == jnox_packet_active_vq(1) && vq->num_free == QUEUE_SIZE) return NULL;
    if (!active(vq)) __builtin_trap();
    for (unsigned i = 0; i < QUEUE_SIZE; ++i) {
        struct rx_slot *slot = &slots[i];
        if (!slot->token) continue;
        if (slot->token != retired_epoch || !slot->map || !slot->map->posted ||
            !stats[2] || vq->num_free >= QUEUE_SIZE) __builtin_trap();
        void *cookie = slot->cookie;
        --slot->map->posted;
        *slot = (struct rx_slot){};
        ++vq->num_free;
        --stats[2];
        return cookie;
    }
    return NULL;
}
#endif
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
#ifdef JNOX_PACKET_LIFECYCLE_ABI
    if (stats[2] || jnox_lifecycle_page_refs_v1()) __builtin_trap();
    for (unsigned i = 0; i < MAP_COUNT; ++i)
        if (maps[i].token == retired_token) __builtin_trap();
    retired_epoch = 0;
#endif
    for (unsigned i = 0; i < QUEUE_SIZE; ++i) {
        if (slots[i].token != retired_token) continue;
        slots[i] = (struct rx_slot){};
        --stats[2];
    }
    for (unsigned i = 0; i < MAP_COUNT; ++i)
        if (maps[i].token == retired_token) maps[i] = (struct rx_map){};
    if (running == retired_token) {
        running = 0;
#ifdef JNOX_PACKET_RX_POLL_ABI
        need_rx_notify = false;
#endif
    }
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

#ifdef JNOX_PACKET_RX_POLL_ABI
#include <jnox_rx_napi.h>
#ifdef JNOX_PACKET_RX_IRQ_RACE_ABI
#include <uapi/linux/time_types.h>
#ifndef JNOX_PACKET_RX_IRQ_ABI
#error "The controlled rearm race requires real owner IRQ delivery"
#endif
/* This journal is read by the replay harness using physical RAM snapshots.
 * Host ingress is held at exactly one frame until WAIT, and two until PASS. */
volatile unsigned long long jnox_irq_race_gate[16];
extern int race_clock(int, struct __kernel_timespec *) asm("clock_gettime");
extern int race_sleep(int, int, const struct __kernel_timespec *, struct __kernel_timespec *)
    asm("clock_nanosleep");
extern void jnox_original_rx_irq_stats_v1(unsigned long long out[8]);
extern unsigned long long jnox_rx_catchups_v1(void);

static void race_phase(unsigned phase)
{
    asm volatile("dmb ishst" ::: "memory");
    jnox_irq_race_gate[1] = phase;
}

static __noreturn void race_fail(unsigned reason)
{
    jnox_irq_race_gate[14] = reason;
    race_phase(255);
    call_rx(1, 99, NULL, 0);
    __builtin_unreachable();
}

static u64 race_now(void)
{
    struct __kernel_timespec t;
    if (race_clock(1, &t) || t.tv_sec < 0 || t.tv_nsec < 0 || t.tv_nsec >= 1000000000L)
        race_fail(1);
    return (u64)t.tv_sec * 1000000000ULL + t.tv_nsec;
}

static long race_metric(unsigned index)
{
    long value = call_rx(175, running, (const void *)(unsigned long)index, 0);
    if (value < 0) race_fail(2);
    return value;
}

static void race_after_rearm(unsigned snapshot)
{
    if (jnox_irq_race_gate[1]) return;
    if (snapshot != 1) {
        if (stats[1] || snapshot) race_fail(3);
        return;
    }
    unsigned long long notified[8];
    jnox_original_rx_irq_stats_v1(notified);
    jnox_irq_race_gate[0] = 0x4952515241434531ULL;
    jnox_irq_race_gate[2] = running;
    jnox_irq_race_gate[3] = snapshot;
    long baseline = race_metric(21);
    long head = race_metric(18);
    jnox_irq_race_gate[4] = baseline;
    jnox_irq_race_gate[11] = head;
    if (stats[1] != 1 || baseline != 1 || head != 1 ||
        notified[3] != 1 || notified[5] != 1 || jnox_rx_catchups_v1() ||
        race_metric(10) || (unsigned long)race_metric(19) != running) race_fail(4);
    u64 start = race_now();
    jnox_irq_race_gate[5] = start;
    race_phase(1);
    /* Yield even if the host injects immediately upon seeing WAIT. Physical
     * IRQs arrive normally; this loop never consumes or acknowledges one. */
    for (;;) {
        u64 now = race_now();
        if (now - start >= 10000000000ULL) race_fail(5);
        u64 target = now + 1000000ULL;
        struct __kernel_timespec t = { .tv_sec = target / 1000000000ULL,
                                      .tv_nsec = target % 1000000000ULL };
        if (race_sleep(1, 1, &t, NULL)) race_fail(6);
        ++jnox_irq_race_gate[15];
        long seen = race_metric(21);
        if (seen == baseline) continue;
        u64 observed = race_now();
        if (observed - start >= 10000000000ULL) race_fail(5);
        long advanced_head = race_metric(18);
        unsigned long generation = race_metric(19);
        jnox_irq_race_gate[6] = seen;
        jnox_irq_race_gate[7] = observed;
        jnox_irq_race_gate[12] = advanced_head;
        jnox_irq_race_gate[13] = generation;
        if (seen != baseline + 1 || advanced_head != 2 || race_metric(17) != 1 ||
            generation != running || stats[1] != 1 || race_metric(10)) race_fail(7);
        race_phase(2);
        return;
    }
}

static void race_after_poll(unsigned snapshot, bool pending)
{
    if (jnox_irq_race_gate[1] != 2) return;
    jnox_irq_race_gate[8] = snapshot;
    jnox_irq_race_gate[9] = pending;
    jnox_irq_race_gate[10] = race_now();
    if (snapshot != jnox_irq_race_gate[3] || !pending || stats[1] != 1 ||
        race_metric(21) != 2 || (unsigned long)race_metric(19) != jnox_irq_race_gate[2])
        race_fail(8);
    race_phase(3);
}
#endif

static long rx_event(struct virtqueue *vq, unsigned long mode, unsigned arg)
{
    if (!active(vq) || running != jnox_packet_token()) {
        fault(-ENODEV);
        call_rx(1, 98, NULL, 0);
        __builtin_unreachable();
    }
    long rc = call_rx(173, running, (void *)mode, arg);
    if (rc < 0 || (mode == 1 ? rc > 65535 : mode == 2 ? rc > 1 : rc != 0)) {
        fault(rc < 0 ? rc : -EPROTO);
        call_rx(1, 98, NULL, 0);
        __builtin_unreachable();
    }
    jnox_rx_napi_event_v1(mode, rc);
    return rc;
}

void jnox_rx_disable_cb_v1(struct virtqueue *vq)
{
    rx_event(vq, 0, 0);
}

unsigned virtqueue_enable_cb_prepare(struct virtqueue *vq)
{
    unsigned snapshot = rx_event(vq, 1, 0);
#ifdef JNOX_PACKET_RX_IRQ_RACE_ABI
    race_after_rearm(snapshot);
#endif
    return snapshot;
}

bool virtqueue_poll(struct virtqueue *vq, unsigned snapshot)
{
    bool pending = rx_event(vq, 2, snapshot) != 0;
#ifdef JNOX_PACKET_RX_IRQ_RACE_ABI
    race_after_poll(snapshot, pending);
#endif
    return pending;
}

bool jnox_rx_kick_prepare_v1(struct virtqueue *vq)
{
    return active(vq) && running == jnox_packet_token() && need_rx_notify;
}

bool jnox_rx_notify_v1(struct virtqueue *vq)
{
    if (!jnox_rx_kick_prepare_v1(vq)) return false;
    rx_event(vq, 3, 0);
    need_rx_notify = false;
    return true;
}

unsigned int virtqueue_get_vring_size(const struct virtqueue *vq)
{
    if (!active(vq)) {
        fault(-ENODEV);
        return 0;
    }
    return QUEUE_SIZE;
}
#endif
#ifdef JNOX_PACKET_LIFECYCLE_ABI
static bool lifecycle_queue(const struct virtqueue *vq)
{
    return vq && (vq == jnox_packet_active_vq(0) || vq == jnox_packet_active_vq(1)) &&
           jnox_packet_token() && !retired_epoch;
}
bool virtqueue_is_broken(const struct virtqueue *vq)
{
    if (!lifecycle_queue(vq)) return true;
    long status = call_rx(160, 0, NULL, 0);
    return status < 0 || (status & (VIRTIO_CONFIG_S_FAILED | VIRTIO_CONFIG_S_NEEDS_RESET));
}
struct device *virtqueue_dma_dev(struct virtqueue *vq)
{
    if (!lifecycle_queue(vq) || !vq->vdev) return NULL;
    return &vq->vdev->dev;
}
bool virtqueue_kick(struct virtqueue *vq)
{
    if (virtqueue_is_broken(vq)) return false;
    return !virtqueue_kick_prepare(vq) || virtqueue_notify(vq);
}
#endif
