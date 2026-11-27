// SPDX-License-Identifier: GPL-2.0-or-later
#include <jnox_virtnet_layout.h>
#include <linux/cpuhotplug.h>
#include <linux/cpu.h>
#include <linux/dim.h>
#include <net/xdp.h>
#ifdef JNOX_LIFECYCLE_FAULT_TEST
#include <lifecycle_faults.h>
#endif
#include "lifecycle_platform.h"

atomic_t __num_online_cpus = ATOMIC_INIT(1);
static unsigned int topology_readers;
static cpumask_t local_mask;
static bool mask_in_use;
void cpus_read_lock(void) { ++topology_readers; barrier(); }
void cpus_read_unlock(void)
{
    if (!topology_readers) __builtin_trap();
    barrier();
    --topology_readers;
}
bool alloc_cpumask_var_node(cpumask_var_t *out, gfp_t flags, int node)
{
    if (!out || mask_in_use || (node != NUMA_NO_NODE && node != 0)) return false;
    mask_in_use = true;
    if (flags & __GFP_ZERO) memset(&local_mask, 0, sizeof(local_mask));
    *out = &local_mask;
    return true;
}
void free_cpumask_var(cpumask_var_t mask)
{
    if (!mask) return;
    if (!mask_in_use || mask != &local_mask) __builtin_trap();
    mask_in_use = false;
}

typedef int (*multi_fn)(unsigned int, struct hlist_node *);
struct cpu_state_record {
    bool registered;
    multi_fn up, down;
    struct hlist_head nodes;
};
static struct cpu_state_record online, dead;
static struct cpu_state_record *cpu_state(enum cpuhp_state state)
{
    if (state == CPUHP_AP_ONLINE_DYN) return &online;
    if (state == CPUHP_VIRT_NET_DEAD) return &dead;
    return NULL;
}
int __cpuhp_setup_state(enum cpuhp_state id, const char *name, bool invoke,
                        int (*up)(unsigned), int (*down)(unsigned), bool multi)
{
    struct cpu_state_record *s = cpu_state(id);
    if (!s || s->registered || !name || !multi || invoke || (!up && !down)) return -EINVAL;
    s->up = (multi_fn)(void *)up;
    s->down = (multi_fn)(void *)down;
    s->registered = true;
    return s == &online ? id : 0;
}
void __cpuhp_remove_state(enum cpuhp_state id, bool invoke)
{
    struct cpu_state_record *s = cpu_state(id);
    if (!s || !s->registered || invoke || s->nodes.first) __builtin_trap();
    *s = (struct cpu_state_record){};
}
int __cpuhp_state_add_instance(enum cpuhp_state id, struct hlist_node *node, bool invoke)
{
    struct cpu_state_record *s = cpu_state(id);
    if (!s || !s->registered || !node || node->pprev || s->nodes.first) return -EINVAL;
#ifdef JNOX_LIFECYCLE_FAULT_TEST
    /* virtnet_cpu_notif_add installs the online node first. Match the
     * second call only when the sibling really belongs to that instance. */
    if (s == &dead && !invoke) {
        struct virtnet_info *vi = container_of(node, struct virtnet_info, node_dead);
        if (online.nodes.first == &vi->node &&
            jnox_lifecycle_fault_hit_v1(JF_CPU_DEAD, node, __builtin_return_address(0)))
            return -ENOMEM;
    }
#endif
    if (invoke && s->up) {
        int rc = s->up(0, node);
        if (rc) return rc;
    }
    hlist_add_head(node, &s->nodes);
    return 0;
}
int __cpuhp_state_remove_instance(enum cpuhp_state id, struct hlist_node *node, bool invoke)
{
    struct cpu_state_record *s = cpu_state(id);
    if (!s || !s->registered || !node || s->nodes.first != node) return -EINVAL;
    if (invoke && s->down) {
        int rc = s->down(0, node);
        if (rc) return rc;
    }
    hlist_del_init(node);
    return 0;
}

static struct xdp_rxq_info *registered_rx;
static bool shared_model;
#ifdef JNOX_LIFECYCLE_FAULT_TEST
void jnox_lifecycle_fault_read_platform_v1(volatile unsigned long long out[JF_FIELDS])
{
    out[JF_ONLINE_NODE] = (unsigned long)online.nodes.first;
    out[JF_DEAD_NODE] = (unsigned long)dead.nodes.first;
    out[JF_REGISTERED_RXQ] = (unsigned long)registered_rx;
    out[JF_SHARED_MODEL] = shared_model;
}
#endif
int __xdp_rxq_info_reg(struct xdp_rxq_info *x, struct net_device *dev, u32 index,
                       unsigned int napi_id, u32 frag_size)
{
    if (!dev || index || registered_rx) return -EINVAL;
    struct virtnet_info *vi = netdev_priv(dev);
    if (!vi->rq || x != &vi->rq->xdp_rxq || !napi_id || napi_id != vi->rq->napi.napi_id)
        return -EINVAL;
    *x = (struct xdp_rxq_info){.dev=dev, .queue_index=index, .frag_size=frag_size,
                               .reg_state=1};
    registered_rx = x;
    return 0;
}
int xdp_rxq_info_reg_mem_model(struct xdp_rxq_info *x, enum xdp_mem_type type, void *allocator)
{
    if (!x || x != registered_rx || shared_model || allocator || type != MEM_TYPE_PAGE_SHARED)
        return -EINVAL;
#ifdef JNOX_LIFECYCLE_FAULT_TEST
    if (jnox_lifecycle_fault_hit_v1(JF_XDP_MODEL, x, __builtin_return_address(0)))
        return -ENOMEM;
#endif
    x->mem.type = type;
    x->mem.id = 0;
    shared_model = true;
    return 0;
}
void xdp_rxq_info_unreg(struct xdp_rxq_info *x)
{
    if (!x || x != registered_rx) __builtin_trap();
    *x = (struct xdp_rxq_info){ .reg_state = 2 };
    registered_rx = NULL;
    shared_model = false;
}

/* The lifecycle transport negotiates no VQ_NOTF_COAL control queue.
 * Upstream gates creation, per-RQ setup, cancellation and release on that
 * bit. Refuse creation before any work or moderation state is allocated. */
volatile unsigned int jnox_lifecycle_dim_fault;
static __noreturn void unexpected_dim(unsigned int operation)
{
    WRITE_ONCE(jnox_lifecycle_dim_fault, operation);
    __builtin_trap();
}

int net_dim_init_irq_moder(struct net_device *dev, u8 profiles, u8 flags,
                           u8 rx_mode, u8 tx_mode, work_func_t rx, work_func_t tx)
{
    WRITE_ONCE(jnox_lifecycle_dim_fault, 1);
    return -EOPNOTSUPP;
}

void net_dim_setting(struct net_device *dev, struct dim *dim, bool tx)
{
    unexpected_dim(2);
}

struct dim_cq_moder net_dim_get_rx_irq_moder(struct net_device *dev, struct dim *dim)
{
    unexpected_dim(3);
}

void net_dim_work_cancel(struct dim *dim)
{
    unexpected_dim(4);
}

void net_dim_free_irq_moder(struct net_device *dev)
{
    unexpected_dim(5);
}
