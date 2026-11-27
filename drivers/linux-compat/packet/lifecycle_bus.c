/* SPDX-License-Identifier: GPL-2.0-or-later */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <linux/module.h>
#include <jnox_tx_virtqueue.h>
#include <lifecycle_bus.h>
#include <lifecycle_netdev.h>
#include "lifecycle_platform.h"

extern struct virtio_driver virtio_net_driver;
extern struct virtio_device *jnox_net_validated_device(void);
extern int init_module(void);
extern void cleanup_module(void);
extern void jnox_lifecycle_devres_release_v1(struct device *);
extern void jnox_net_forget_queues(void);
extern int jnox_lifecycle_bind_worker_v1(struct virtio_device *);
extern void jnox_lifecycle_unbind_worker_v1(void);

struct module __this_module = {
    .name = "virtio_net",
    .state = MODULE_STATE_UNFORMED,
    .init = init_module,
#ifdef CONFIG_MODULE_UNLOAD
    .exit = cleanup_module,
#endif
};

static struct virtio_device *matched;
static bool probed, opened, reset_done;
struct virtio_driver *jnox_lifecycle_registered_driver;
extern bool napi_tx;
/* Address, pre-selection value, readback, selections, TX NAPI address,
 * observed weight and last successfully checked probe epoch. */
volatile unsigned long long jnox_lifecycle_tx_param[7];

int __register_virtio_driver(struct virtio_driver *d, struct module *owner)
{
    if (jnox_lifecycle_registered_driver || d != &virtio_net_driver || !d->validate || !d->probe ||
        !d->remove || !d->id_table) return -EINVAL;
    if (owner != &__this_module || owner->state != MODULE_STATE_COMING)
        return -EINVAL;
    d->driver.owner = owner;
    jnox_lifecycle_registered_driver = d;
    return 0;
}

void unregister_virtio_driver(struct virtio_driver *d)
{
    if (d != jnox_lifecycle_registered_driver || probed ||
        d->driver.owner != &__this_module || __this_module.state != MODULE_STATE_GOING)
        __builtin_trap();
    d->driver.owner = NULL;
    jnox_lifecycle_registered_driver = NULL;
}

static unsigned long long stats[12];
volatile long jnox_lifecycle_bus_fault;

static long svc(unsigned long number, unsigned long arg, unsigned long val)
{
    register unsigned long op asm("x8") = number;
    register long rc asm("x0") = arg;
    register unsigned long b asm("x1") = val;
    register unsigned long c asm("x2") = 0;
    asm volatile("svc #0x4a01" : "+r"(rc), "+r"(b), "+r"(c) : "r"(op) : "memory", "cc");
    return rc;
}

/* The Linux config API cannot propagate an error from a void operation.
 * Leave teardown to the kernel owner-exit path on a failed device transition. */
static __noreturn void fault(long err)
{
    WRITE_ONCE(jnox_lifecycle_bus_fault, err);
    svc(1, 98, 0);
    __builtin_trap();
}

static int start_provider(void)
{
    if (__this_module.state != MODULE_STATE_UNFORMED) return -EBUSY;
    __this_module.state = MODULE_STATE_COMING;
    int err = __this_module.init();
    if (err && jnox_lifecycle_registered_driver) fault(-EPROTO);
    if (!err && jnox_lifecycle_registered_driver != &virtio_net_driver) fault(-EPROTO);
    __this_module.state = err ? MODULE_STATE_UNFORMED : MODULE_STATE_LIVE;
    return err;
}

static void stop_provider(void)
{
    if (__this_module.state != MODULE_STATE_LIVE || probed) fault(-EBUSY);
    __this_module.state = MODULE_STATE_GOING;
    cleanup_module();
    if (jnox_lifecycle_registered_driver) fault(-EPROTO);
    __this_module.state = MODULE_STATE_UNFORMED;
}

bool jnox_lifecycle_feature_v1(unsigned int bit)
{
    const struct virtio_driver *d = &virtio_net_driver;
    for (unsigned int i = 0; i < d->feature_table_size; ++i)
        if (bit == d->feature_table[i]) return true;
    for (unsigned int i = 0; i < d->feature_table_size_legacy; ++i)
        if (bit == d->feature_table_legacy[i]) return true;
    return false;
}

int jnox_lifecycle_validate_v1(struct virtio_device *v)
{
    if (probed || v->priv) return -EBUSY;
    long id = svc(178, 0, 0), vendor = svc(178, 1, 0);
    if (id < 0 || vendor < 0) return id < 0 ? id : vendor;
    if (!jnox_lifecycle_registered_driver) {
        jnox_lifecycle_tx_param[0] = (unsigned long)&napi_tx;
        jnox_lifecycle_tx_param[1] = READ_ONCE(napi_tx);
        WRITE_ONCE(napi_tx, false);
        bool selected = READ_ONCE(napi_tx);
        jnox_lifecycle_tx_param[2] = selected;
        if (selected) return -EPROTO;
        ++jnox_lifecycle_tx_param[3];
        int err = start_provider();
        if (err) return err;
    }
    const struct virtio_device_id *item = jnox_lifecycle_registered_driver->id_table;
    for (; item->device; ++item) {
        if (item->device != id || (item->vendor != VIRTIO_DEV_ANY_ID && item->vendor != vendor))
            continue;
        v->id.device = id;
        v->id.vendor = vendor;
        v->dev.driver = &virtio_net_driver.driver;
        stats[0] = (unsigned long)&virtio_net_driver;
        stats[1] = (unsigned long)item;
        stats[2] = (unsigned long)virtio_net_driver.validate;
        int err = jnox_lifecycle_registered_driver->validate(v);
        if (err) stop_provider();
        matched = err ? NULL : v;
        return err;
    }
    matched = NULL;
    stop_provider();
    return -ENODEV;
}

void jnox_lifecycle_get_config_v1(struct virtio_device *v, unsigned int off, void *dst, unsigned int len)
{
    if (v != matched || !dst || !len) fault(-EINVAL);
    if (off < ETH_ALEN && len <= ETH_ALEN - off) {
        for (unsigned i = 0; i < len; ++i) {
            long b = svc(158, off+i, 1);
            if (b < 0 || b > 255) fault(b < 0 ? b : -EPROTO);
            ((u8 *)dst)[i] = b;
        }
    } else if (off == offsetof(struct virtio_net_config, mtu) && len == 2) {
        long mtu = svc(158, off, 2);
        if (mtu < 0 || mtu > 65535) fault(mtu < 0 ? mtu : -EPROTO);
        __le16 value = cpu_to_le16(mtu);
        memcpy(dst, &value, sizeof(value));
    } else fault(-EOPNOTSUPP);
}

u8 jnox_lifecycle_get_status_v1(struct virtio_device *v)
{
    if (v != matched) fault(-ENODEV);
    long s = svc(160, 0, 0);
    if (s < 0 || s > 255) fault(s < 0 ? s : -EPROTO);
    return s;
}

void jnox_lifecycle_set_status_v1(struct virtio_device *v, u8 status)
{
    if (v != matched || status != 15 || jnox_lifecycle_get_status_v1(v) != 11 ||
        !jnox_lifecycle_registered_v1()) fault(-EPROTO);
    int err = jnox_rx_lifecycle_ready_v1();
    if (err || jnox_lifecycle_get_status_v1(v) != 15) fault(err ? err : -EPROTO);
}

/* The sole native FIFO worker serializes these transitions and callbacks.
 * EL1 IRQ handling publishes events; it never enters this device in EL0. */
void virtio_config_changed(struct virtio_device *v)
{
    if (!v || v != matched) fault(-ENODEV);
    if (!v->config_core_enabled || v->config_driver_disabled) {
        v->config_change_pending = true;
        return;
    }
    struct virtio_driver *d = jnox_lifecycle_registered_driver;
    if (!d || v->dev.driver != &d->driver) fault(-EPROTO);
    if (d->config_changed) {
        d->config_changed(v);
        v->config_change_pending = false;
    }
}

void virtio_config_driver_disable(struct virtio_device *v)
{
    if (v != matched) fault(-ENODEV);
    v->config_driver_disabled = true;
}

void virtio_config_driver_enable(struct virtio_device *v)
{
    if (v != matched) fault(-ENODEV);
    v->config_driver_disabled = false;
    if (v->config_change_pending) virtio_config_changed(v);
}

void jnox_lifecycle_reset_v1(struct virtio_device *v)
{
    if (v != matched || reset_done) fault(-EPROTO);
    long err = svc(156, 0, 0);
    if (err) fault(err);
    if (jnox_packet_token()) jnox_rx_lifecycle_retire_v1(jnox_packet_token());
    reset_done = true;
}

void virtio_reset_device(struct virtio_device *v)
{
    v->config->reset(v);
}

void jnox_lifecycle_del_vqs_v1(struct virtio_device *v)
{
    if (v != matched || !reset_done) fault(-EPROTO);
    jnox_net_forget_queues();
}

static int probe(void)
{
    struct virtio_device *v = jnox_net_validated_device();
    if (!v || v != matched) return -ENODEV;
    if (probed || opened || v->priv || jnox_packet_token()) return -EBUSY;
    if (jnox_lifecycle_get_status_v1(v) != 11) return -EPROTO;
    reset_done = false;
    stats[3] = (unsigned long)virtio_net_driver.probe;
    v->config_core_enabled = false;
    int err = jnox_lifecycle_registered_driver->probe(v);
    if (err) {
        if (!reset_done) jnox_lifecycle_reset_v1(v);
        jnox_net_forget_queues();
        jnox_lifecycle_devres_release_v1(&v->dev);
        stop_provider();
        memset(v, 0, sizeof(*v));
        matched = NULL;
        return err;
    }
    struct net_device *net = jnox_lifecycle_registered_v1();
    struct virtnet_info *vi = v->priv;
    if (!net || !vi || vi != netdev_priv(net) || vi->dev != net ||
        !net->netdev_ops || !net->netdev_ops->ndo_open || !net->netdev_ops->ndo_stop ||
        !jnox_packet_token()) fault(-EPROTO);
    if (!vi->sq || !vi->sq->napi.poll || vi->sq->napi.dev != net)
        fault(-EPROTO);
    jnox_lifecycle_tx_param[4] = (unsigned long)&vi->sq->napi;
    jnox_lifecycle_tx_param[5] = READ_ONCE(vi->sq->napi.weight);
    if (READ_ONCE(napi_tx) || jnox_lifecycle_tx_param[5]) fault(-EPROTO);
    jnox_lifecycle_tx_param[6] = jnox_packet_token();
    stats[4] = (unsigned long)net->netdev_ops->ndo_open;
    stats[5] = (unsigned long)net->netdev_ops->ndo_stop;
    stats[6] = (unsigned long)virtio_net_driver.remove;
    err = jnox_lifecycle_bind_worker_v1(v);
    if (err) fault(err);
    v->config_core_enabled = true;
    if (v->config_change_pending) virtio_config_changed(v);
    jnox_lifecycle_run_work_v1();
    probed = true;
    ++stats[7];
    stats[11] = jnox_packet_token();
    return 0;
}

static int open_device(void)
{
    if (!probed || !matched) return -ENODEV;
    if (opened) return -EBUSY;
    int err = jnox_lifecycle_net_up_v1(jnox_lifecycle_registered_v1());
    if (!err) {
        jnox_lifecycle_run_work_v1();
        opened = true;
        ++stats[8];
    }
    return err;
}

static int close_device(void)
{
    if (!opened) return -EINVAL;
    int err = jnox_lifecycle_net_down_v1(jnox_lifecycle_registered_v1());
    if (!err) {
        jnox_lifecycle_run_work_v1();
        opened = false;
        ++stats[9];
    }
    return err;
}

bool jnox_lifecycle_is_up_v1(void) { return opened; }

static int remove_device(void)
{
    if (!probed) return -ENODEV;
    if (opened) {
        int err = close_device();
        if (err) return err;
    }
    matched->config_core_enabled = false;
    jnox_lifecycle_unbind_worker_v1();
    jnox_lifecycle_registered_driver->remove(matched);
    jnox_lifecycle_devres_release_v1(&matched->dev);
    if (!reset_done || jnox_packet_token() || jnox_lifecycle_registered_v1())
        fault(-EPROTO);
    memset(matched, 0, sizeof(*matched));
    matched = NULL;
    probed = false;
    stop_provider();
    ++stats[10];
    return 0;
}

static int call_in_context(int (*operation)(void))
{
    int err = jnox_packet_context_enter_v1(0);
    if (err) return err;
    err = operation();
    jnox_packet_context_leave_v1();
    return err;
}

int jnox_lifecycle_probe_v1(void) { return call_in_context(probe); }
int jnox_lifecycle_open_v1(void) { return call_in_context(open_device); }
int jnox_lifecycle_close_v1(void) { return call_in_context(close_device); }
int jnox_lifecycle_remove_v1(void) { return call_in_context(remove_device); }

void jnox_lifecycle_stats_v1(unsigned long long out[12])
{
    memcpy(out, stats, sizeof(stats));
}
