#include <jnox_linux_compat.h>
#include <jnox_pl031.h>

#define DEVICE_BASE 0x09010000UL
#define PAGE_BYTES 4096
#define RTC_IRQ 34

static struct amba_device device = {
    .res = { DEVICE_BASE, DEVICE_BASE + PAGE_BYTES - 1 }, .irq = { RTC_IRQ }
};
static struct jnox_pl031_stats stats;
static struct amba_driver *bound;
static struct rtc_device *rtc;
static irq_handler_t handler;
static void *handler_data;
static bool claimed, regions, mapped, registered;
static unsigned allocations;
static _Alignas(16) unsigned char pool[4][128];
static int io_error;

static long svc(unsigned long op, unsigned long a, unsigned long b)
{
    register unsigned long x8 __asm__("x8") = op;
    register unsigned long x0 __asm__("x0") = a;
    register unsigned long x1 __asm__("x1") = b;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1) : "r"(x8) : "memory","cc");
    return (long)x0;
}

static void remember_error(int err)
{
    if (!io_error)
        io_error = err;
}

static long access_reg(unsigned op, unsigned long address, unsigned value)
{
    if (!claimed || address < DEVICE_BASE || address > DEVICE_BASE + PAGE_BYTES - 4 ||
        ((address - DEVICE_BASE) & 3))
        return -22;
    return svc(op, address - DEVICE_BASE, value);
}

uint32_t readl(const void *p)
{
    long result = mapped ? access_reg(152, (unsigned long)p, 0) : -19;
    if (result < 0) {
        remember_error((int)result);
        return 0;
    }
    return result;
}

void writel(uint32_t value, void *p)
{
    long ret = mapped ? access_reg(153, (unsigned long)p, value) : -19;
    if (ret < 0)
        remember_error(ret);
}

int amba_request_regions(struct amba_device *dev, const char *name)
{
    (void)name;
    if (dev != &device || !claimed) return -19;
    if (regions) return -16;
    regions = true;
    return 0;
}

void amba_release_regions(struct amba_device *dev)
{
    if (dev == &device) regions = false;
}

void *devm_kzalloc(struct device *dev, size_t size, int flags)
{
    if (dev != &device.dev || !regions || flags != GFP_KERNEL || size > sizeof(pool[0]) ||
        !size || allocations >= sizeof(pool) / sizeof(pool[0])) return NULL;
    unsigned char *ptr = pool[allocations++];
    for (size_t i = 0; i < size; i++) ptr[i] = 0;
    return ptr;
}

void *devm_kmemdup(struct device *dev, const void *source, size_t size, int flags)
{
    unsigned char *ptr = devm_kzalloc(dev, size, flags);
    if (ptr)
        for (size_t i = 0; i < size; ++i) ptr[i] = ((const unsigned char *)source)[i];
    return ptr;
}

void *devm_ioremap(struct device *dev, unsigned long start, unsigned long size)
{
    if (dev != &device.dev || !regions || mapped || start != DEVICE_BASE || size != PAGE_BYTES)
        return NULL;
    mapped = true;
    /* Only readl/writel interpret this token. EL0 never dereferences it. */
    return (void *)start;
}

struct rtc_device *devm_rtc_allocate_device(struct device *dev)
{
    if (rtc) return (void *)-16L;
    rtc = devm_kzalloc(dev, sizeof(*rtc), GFP_KERNEL);
    if (!rtc) return (void *)-12L;
    rtc->features[0] = 1UL << RTC_FEATURE_ALARM;
    return rtc;
}

int devm_rtc_register_device(struct rtc_device *r)
{
    if (!r || r != rtc || !r->ops || !mapped) return -22;
    if (registered) return -16;
    registered = true;
    return 0;
}

/* The driver treats suspend/wake configuration as optional. */
int devm_device_init_wakeup(struct device *dev) { (void)dev; return -95; }
int devm_pm_set_wake_irq(struct device *dev, int irq) { (void)dev; (void)irq; return -95; }

int request_irq(unsigned irq, irq_handler_t fn, unsigned long flags, const char *name, void *data)
{
    (void)name;
    if (flags) return -95;
    if (irq != RTC_IRQ || !fn || !data || !registered) return -22;
    if (handler) return -16;
    handler = fn;
    handler_data = data;
    return 0;
}

void free_irq(unsigned irq, void *data)
{
    if (irq != RTC_IRQ || data != handler_data || !handler) {
        remember_error(-22);
        return;
    }
    handler = NULL;
    handler_data = NULL;
}

void rtc_update_irq(struct rtc_device *r, unsigned long count, unsigned long flags)
{
    if (!registered || rtc != r || !handler) {
        remember_error(-19);
        return;
    }
    stats.irq_events += count;
    stats.irq_flags = flags;
}

static int primecell_id(unsigned offset, uint32_t *id)
{
    *id = 0;
    for (unsigned i = 0; i < 4; ++i) {
        long byte = access_reg(152, DEVICE_BASE + offset + 4 * i, 0);
        if (byte < 0) return byte;
        *id |= ((uint32_t)byte & 255u) << (i * 8);
    }
    return 0;
}

static int release(void)
{
    handler = NULL;
    handler_data = NULL;
    registered = false;
    rtc = NULL;
    device.dev.driver_data = NULL;
    mapped = false;
    regions = false;
    allocations = 0;
    bound = NULL;
    int result = svc(155, 0, 0);
    if (result == 0) claimed = false;
    return result;
}

int jnox_pl031_bind(void)
{
    if (claimed) return -16;
    int ret = svc(155, 1, 0);
    if (ret) return ret;
    claimed = true;
    io_error = 0;
    ret = primecell_id(0xfe0, &stats.periphid);
    if (!ret) ret = primecell_id(0xff0, &stats.cellid);
    struct amba_driver *driver = jnox_linux_amba_driver();
    if (!ret && (stats.cellid != 0xb105f00d || (stats.periphid & 0xfffff) != 0x41031))
        ret = -19;
    if (!ret) {
        const struct amba_id *id = driver->id_table;
        while (id->mask && (stats.periphid & id->mask) != id->id) ++id;
        if (!id->mask) ret = -19;
        else {
            device.periphid = stats.periphid;
            ++stats.probes;
            ret = driver->probe(&device, id);
            if (!ret) bound = driver;
        }
    }
    if (!ret) ret = io_error;
    if (ret) {
        if (bound) { bound->remove(&device); ++stats.removes; }
        int err = release();
        if (err) return err;
    }
    return ret;
}

static int ready(void)
{
    io_error = 0;
    return bound && registered ? 0 : -19;
}

static int result(int ret) { return io_error ? io_error : ret; }

int jnox_pl031_read_time(uint32_t *seconds)
{
    if (!seconds) return -22;
    if (ready()) return -19;
    struct rtc_time tm;
    int ret = result(rtc->ops->read_time(&device.dev, &tm));
    if (ret) return ret;
    time64_t n = rtc_tm_to_time64(&tm);
    if (n < 0 || n > U32_MAX) return -34;
    *seconds = n;
    return 0;
}

int jnox_pl031_set_time(uint32_t seconds)
{
    if (ready()) return -19;
    struct rtc_time tm;
    rtc_time64_to_tm(seconds, &tm);
    return result(rtc->ops->set_time(&device.dev, &tm));
}

int jnox_pl031_read_alarm(uint32_t *seconds, unsigned *enabled, unsigned *pending)
{
    if (!seconds || !enabled || !pending) return -22;
    if (ready()) return -19;
    struct rtc_wkalrm alarm;
    int ret = result(rtc->ops->read_alarm(&device.dev, &alarm));
    if (ret) return ret;
    time64_t n = rtc_tm_to_time64(&alarm.time);
    if (n < 0 || n > U32_MAX) return -34;
    *seconds = n;
    *enabled = alarm.enabled;
    *pending = alarm.pending;
    return 0;
}

int jnox_pl031_set_alarm(uint32_t seconds, unsigned enabled)
{
    if (enabled > 1) return -22;
    if (ready()) return -19;
    struct rtc_wkalrm alarm = { .enabled = enabled };
    rtc_time64_to_tm(seconds, &alarm.time);
    return result(rtc->ops->set_alarm(&device.dev, &alarm));
}

int jnox_pl031_poll_irq(void)
{
    if (ready() || !handler) return -19;
    long pending = svc(154, 0, 0);
    if (pending <= 0) return pending;
    if (pending != 1) return -5;
    ++stats.irq_calls;
    return result(handler(RTC_IRQ, handler_data) == IRQ_HANDLED);
}

int jnox_pl031_unbind(void)
{
    if (!claimed) return -19;
    int ret = 0;
    if (bound) {
        io_error = 0;
        ret = rtc->ops->alarm_irq_enable(&device.dev, 0);
        bound->remove(&device);
        ++stats.removes;
        ret = result(ret);
    }
    int err = release();
    return err ? err : ret;
}

void jnox_pl031_stats(struct jnox_pl031_stats *out)
{
    stats.resources = claimed + regions + mapped + allocations + registered + (handler != NULL);
    if (out) *out = stats;
}
