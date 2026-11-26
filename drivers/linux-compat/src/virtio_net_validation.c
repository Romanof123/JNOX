// SPDX-License-Identifier: GPL-2.0-or-later
/* Native single-threaded validation entry for Linux v6.18.65 virtio-net. */
#include <linux/device.h>
#include <linux/errno.h>
#include <linux/virtio.h>
#include <linux/virtio_config.h>
#include <linux/virtio_net.h>

extern int virtnet_validate(struct virtio_device *vdev);

static struct virtio_device device;
static struct {
    bool active;
    long error;
    unsigned mtu, reads, diagnostics;
} run;

static void reject(long error)
{
    if (!run.error)
        run.error = error;
}

static long native_call(unsigned long op, unsigned long arg0, unsigned long arg1)
{
    register unsigned long x8 asm("x8") = op;
    register unsigned long x0 asm("x0") = arg0;
    register unsigned long x1 asm("x1") = arg1;
    asm volatile("svc #0x4a01" : "+r"(x0), "+r"(x1) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

int dynamic_might_resched(void)
{
    /* Yield the current native process. The native ABI does not report
     * whether another process actually ran. */
    long result = native_call(8, 0, 0);
    if (result != 0)
        reject(result < 0 ? result : -EPROTO);
    return 0;
}

void virtio_check_driver_offered_feature(const struct virtio_device *vdev,
                                       unsigned int bit)
{
    /* All the device features inspected by the pinned callback are entries
     * in upstream VIRTNET_FEATURES. Transport bits bypass this Linux hook. */
    if (vdev != &device || !run.active) {
        reject(-ENODEV);
        return;
    }
    switch (bit) {
    case VIRTIO_NET_F_CTRL_VQ:
    case VIRTIO_NET_F_CTRL_RX:
    case VIRTIO_NET_F_CTRL_VLAN:
    case VIRTIO_NET_F_GUEST_ANNOUNCE:
    case VIRTIO_NET_F_MQ:
    case VIRTIO_NET_F_CTRL_MAC_ADDR:
    case VIRTIO_NET_F_RSS:
    case VIRTIO_NET_F_HASH_REPORT:
    case VIRTIO_NET_F_NOTF_COAL:
    case VIRTIO_NET_F_VQ_NOTF_COAL:
    case VIRTIO_NET_F_MTU:
    case VIRTIO_NET_F_STANDBY:
    case VIRTIO_NET_F_MAC:
        return;
    default:
        reject(-EOPNOTSUPP);
    }
}

static void diagnosis(const struct device *dev, const char *fmt)
{
    if (!run.active || dev != &device.dev || !fmt) {
        reject(-EPROTO);
        return;
    }
    if (run.diagnostics != ~0u)
        ++run.diagnostics;
}

void _dev_err(const struct device *dev, const char *fmt, ...)
{
    diagnosis(dev, fmt);
}

void _dev_warn(const struct device *dev, const char *fmt, ...)
{
    diagnosis(dev, fmt);
}

static void read_config(struct virtio_device *vdev, unsigned offset, void *buf, unsigned len)
{
    /* Only virtnet_validate's 16-bit MTU access is supported. */
    if (!buf || vdev != &device || !run.active ||
        offset != offsetof(struct virtio_net_config, mtu) || len != sizeof(__le16)) {
        reject(-EPROTO);
        return;
    }
    __le16 *out = buf;
    *out = 0;
    if (run.error)
        return;
    ++run.reads;
    long value = native_call(158, offset, len);
    if (value < 0 || value > 0xffff) {
        reject(value < 0 ? value : -EPROTO);
        return;
    }
    run.mtu = value;
    *out = cpu_to_le16((u16)value);
}

static const struct virtio_config_ops config = { .get = read_config };
static const struct virtio_config_ops missing_get;

long jnox_net_validate(unsigned long long *features, unsigned mode, unsigned *mtu,
                       unsigned *reads, unsigned *diagnostics)
{
    const u64 admitted = BIT_ULL(VIRTIO_F_VERSION_1) | BIT_ULL(VIRTIO_NET_F_MAC) |
                         BIT_ULL(VIRTIO_NET_F_MTU);
    if (!features || !mtu || !reads || !diagnostics || mode > 2)
        return -EINVAL;
    if (run.active)
        return -EBUSY;
    run.active = true;
    run.error = 0;
    run.mtu = run.reads = run.diagnostics = 0;
    device.features_array[0] = *features & admitted;
    if (mode == 2)
        __virtio_set_bit(&device, VIRTIO_NET_F_CTRL_RX);
    device.config = mode == 1 ? &missing_get : &config;
    int status = virtnet_validate(&device);
    *features = device.features_array[0];
    *mtu = run.mtu;
    *reads = run.reads;
    *diagnostics = run.diagnostics;
    run.active = false;
    return run.error ? run.error : status;
}
