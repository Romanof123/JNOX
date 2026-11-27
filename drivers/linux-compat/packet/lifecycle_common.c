// SPDX-License-Identifier: GPL-2.0-or-later
/* Linux helpers used by the native, single-threaded virtio-net lifecycle. */
#include <linux/device.h>
#include <linux/find.h>
#include <linux/slab.h>
#include <linux/netdevice.h>
#include <net/net_failover.h>

#define ALLOCATION_BYTES 4096
#define ALLOCATION_COUNT 8

static unsigned char storage[ALLOCATION_COUNT][ALLOCATION_BYTES] __aligned(ALLOCATION_BYTES);
static struct {
    bool allocated;
    struct device *owner;
} allocation[ALLOCATION_COUNT];

unsigned long jnox_lifecycle_common_live;
unsigned long jnox_lifecycle_diagnostic_count;
const char *volatile jnox_lifecycle_last_format;

static void *allocate(size_t size, struct device *owner)
{
    if (!size)
        return ZERO_SIZE_PTR;
    if (size > ALLOCATION_BYTES)
        return NULL;
    for (unsigned int i = 0; i < ALLOCATION_COUNT; ++i) {
        if (!allocation[i].allocated) {
            allocation[i].allocated = true;
            allocation[i].owner = owner;
            ++jnox_lifecycle_common_live;
            return memset(storage[i], 0, ALLOCATION_BYTES);
        }
    }
    return NULL;
}

static bool release(const void *ptr, struct device *owner)
{
    if (ZERO_OR_NULL_PTR(ptr))
        return true;
    for (unsigned int i = 0; i < ALLOCATION_COUNT; ++i) {
        if (allocation[i].allocated && allocation[i].owner == owner && storage[i] == ptr) {
            memset(storage[i], 0, ALLOCATION_BYTES);
            allocation[i].allocated = false;
            allocation[i].owner = NULL;
            --jnox_lifecycle_common_live;
            return true;
        }
    }
    return false;
}

bool jnox_lifecycle_common_free_v1(const void *ptr)
{
    return release(ptr, NULL);
}

void *__kmalloc_noprof(size_t size, gfp_t flags)
{
    return allocate(size, NULL);
}

void *__kvmalloc_node_noprof(DECL_BUCKET_PARAMS(size, b), unsigned long align,
                             gfp_t flags, int node)
{
    if ((node != NUMA_NO_NODE && node != 0) || align > ALLOCATION_BYTES ||
        (align && (align & (align - 1))))
        return NULL;
    return allocate(size, NULL);
}

void *kmemdup_noprof(const void *source, size_t size, gfp_t flags)
{
    void *copy = allocate(size, NULL);
    if (copy && size)
        memcpy(copy, source, size);
    return copy;
}

void kvfree(const void *ptr)
{
    kfree(ptr);
}

void *devm_kmalloc(struct device *dev, size_t size, gfp_t flags)
{
    return dev ? allocate(size, dev) : NULL;
}

void devm_kfree(struct device *dev, const void *ptr)
{
    if (!dev || !release(ptr, dev))
        __builtin_trap();
}

void jnox_lifecycle_devres_release_v1(struct device *owner)
{
    if (!owner)
        __builtin_trap();
    for (unsigned int i = 0; i < ALLOCATION_COUNT; ++i)
        if (allocation[i].allocated && allocation[i].owner == owner)
            release(storage[i], owner);
}

ssize_t sized_strscpy(char *dst, const char *src, size_t count)
{
    if (!count || count > INT_MAX)
        return -E2BIG;
    for (size_t i = 0; i < count; ++i) {
        if (!(dst[i] = src[i]))
            return i;
    }
    dst[count - 1] = '\0';
    return -E2BIG;
}

unsigned long _find_next_bit(const unsigned long *bits, unsigned long size, unsigned long first)
{
    if (first >= size)
        return size;
    unsigned long word = first / BITS_PER_LONG;
    unsigned long last = (size - 1) / BITS_PER_LONG;
    unsigned long mask = ~0UL << (first % BITS_PER_LONG);
    for (;;) {
        unsigned long set = bits[word] & mask;
        if (set)
            return min(size, word * BITS_PER_LONG + (unsigned long)__builtin_ctzl(set));
        if (word == last)
            return size;
        ++word;
        mask = ~0UL;
    }
}

unsigned long _find_first_bit(const unsigned long *bits, unsigned long size)
{
    return _find_next_bit(bits, size, 0);
}

extern int sched_yield(void);

int dynamic_cond_resched(void)
{
    if (sched_yield())
        __builtin_trap();
    return 0;
}

/* Preserve the callsite for diagnostics without using the kernel's formatter. */
static void note(const char *format)
{
    WRITE_ONCE(jnox_lifecycle_last_format, format);
    ++jnox_lifecycle_diagnostic_count;
}

void _dev_info(const struct device *dev, const char *format, ...) { note(format); }
void netdev_err(const struct net_device *dev, const char *format, ...) { note(format); }
void netdev_warn(const struct net_device *dev, const char *format, ...) { note(format); }
void netdev_printk(const char *level, const struct net_device *dev, const char *format, ...)
{
    note(format);
}

extern __noreturn void jnox_lifecycle_nonnull_failover_destroy(void);
void net_failover_destroy(struct failover *failover)
{
    if (failover)
        jnox_lifecycle_nonnull_failover_destroy();
}
