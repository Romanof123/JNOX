// SPDX-License-Identifier: GPL-2.0-or-later
#include <jnox_virtnet_layout.h>
#include <linux/cpuhotplug.h>
#include <linux/cpu.h>
#include <net/xdp.h>
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
