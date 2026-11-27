// SPDX-License-Identifier: GPL-2.0-or-later
#ifdef JNOX_LIFECYCLE_FAULT_TEST
#include <linux/errno.h>
#include <jnox_tx_virtqueue.h>
#include <jnox_rx_virtqueue.h>
#include <jnox_rx_skb.h>
#include <lifecycle_faults.h>
#include "lifecycle_platform.h"

struct virtio_driver;
extern struct virtio_driver *jnox_lifecycle_registered_driver;
extern unsigned long jnox_lifecycle_common_live;

volatile unsigned long long jnox_lifecycle_fault_state[JF_CASES][JF_PHASES][JF_FIELDS];
volatile unsigned long long jnox_lifecycle_fault_events[JF_CASES][JF_EVENTS];
volatile unsigned long long jnox_lifecycle_fault_resources[JF_CASES][JF_PHASES][JF_RESOURCES];

/* Private to the sole FIFO worker in the fault fixture. The peer has its
 * own address space; the kernel and network transport cannot arm hooks. */
static unsigned int active_case, total_fired;

static void snapshot(unsigned int id, unsigned int phase)
{
    volatile unsigned long long *out = jnox_lifecycle_fault_state[id - 1][phase];
    jnox_lifecycle_fault_read_netdev_v1(out);
    jnox_lifecycle_fault_read_platform_v1(out);
    out[JF_QUEUE_TOKEN] = jnox_packet_token();
    out[JF_COMMON_LIVE] = jnox_lifecycle_common_live;
    out[JF_DRIVER] = (unsigned long)jnox_lifecycle_registered_driver;
    out[JF_RX_VQ] = (unsigned long)jnox_packet_active_vq(0);
    out[JF_TX_VQ] = (unsigned long)jnox_packet_active_vq(1);
    unsigned long long queue[8];
    jnox_rx_vq_stats(queue);
    volatile unsigned long long *resources = jnox_lifecycle_fault_resources[id - 1][phase];
    resources[JF_POSTED_RX] = queue[2];
    resources[JF_PAGE_REFS] = jnox_lifecycle_page_refs_v1();
    resources[JF_FIRST_POSTED_VA] = jnox_rx_posted_address_v1(0);
    resources[JF_SKB_PENDING] = jnox_rx_skb_pending_v1();
    resources[JF_TOTAL_POSTED] = queue[0];
    resources[JF_TOTAL_USED] = queue[1];
    resources[JF_TOTAL_READY] = queue[6];
    resources[JF_QUEUE_ERROR] = queue[7];
    volatile unsigned long long *bits = &jnox_lifecycle_fault_events[id - 1][JF_VALID_PHASES];
    __atomic_store_n(bits, *bits | (1ULL << phase), __ATOMIC_RELEASE);
}

int jnox_lifecycle_fault_arm_v1(unsigned int id)
{
    if (!id || id > JF_CASES)
        return -EINVAL;
    if (active_case)
        return -EBUSY;
    volatile unsigned long long *event = jnox_lifecycle_fault_events[id - 1];
    if (event[JF_ARMED_ID])
        return -EALREADY;
    active_case = id;
    event[JF_ARMED_ID] = id;
    snapshot(id, JF_ARMED);
    return 0;
}

int jnox_lifecycle_fault_hit_v1(unsigned int id, const void *candidate, const void *caller)
{
    if (!id || id > JF_CASES || active_case != id)
        return 0;
    volatile unsigned long long *event = jnox_lifecycle_fault_events[id - 1];
    if (event[JF_FIRE_COUNT])
        return 0;
    ++event[JF_FIRE_COUNT];
    event[JF_TOTAL_FIRED] = ++total_fired;
    event[JF_CANDIDATE] = (unsigned long)candidate;
    event[JF_CALLER_PC] = (unsigned long)caller;
    event[JF_FIRED_ID] = id;
    event[JF_ADAPTER_RESULT] = id == JF_ALLOC ? 0 : (long)-ENOMEM;
    snapshot(id, JF_FIRED);
    return 1;
}

int jnox_lifecycle_fault_snapshot_v1(unsigned int id, unsigned int phase)
{
    if (!id || id > JF_CASES || (phase != JF_RETURNED && phase != JF_CLEANED))
        return -EINVAL;
    volatile unsigned long long *event = jnox_lifecycle_fault_events[id - 1];
    if (event[JF_VALID_PHASES] & (1ULL << phase))
        return -EALREADY;
    if (id != active_case || !(event[JF_VALID_PHASES] & (1ULL << (phase - 1))))
        return -EAGAIN;
    snapshot(id, phase);
    if (phase == JF_CLEANED)
        active_case = 0;
    return 0;
}
#endif
