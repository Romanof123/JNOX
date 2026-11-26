#ifndef JNOX_LINUX_COMPAT_H
#define JNOX_LINUX_COMPAT_H
#include <stddef.h>
#include <stdint.h>

typedef _Bool bool;
#define true 1
#define false 0
#define __iomem
#define GFP_KERNEL 0
#define EINVAL 22
#define ENOMEM 12
#define U32_MAX __UINT32_MAX__
typedef __INT64_TYPE__ time64_t;
typedef __UINT64_TYPE__ timeu64_t;
#define RTC_TIMESTAMP_BEGIN_0000 (-62167219200LL)
#define RTC_TIMESTAMP_END_9999 253402300799LL
#define IS_ERR(p) ((unsigned long)(p) >= (unsigned long)-4095)
#define PTR_ERR(p) ((long)(p))
#define IRQF_SHARED 0x80
#define IRQF_COND_SUSPEND 0x40000
#define RTC_FEATURE_ALARM 0
#define RTC_AF 0x20
#define RTC_IRQF 0x80
typedef enum { IRQ_NONE, IRQ_HANDLED } irqreturn_t;
typedef irqreturn_t (*irq_handler_t)(int, void *);
struct device { void *driver_data; };
struct resource { unsigned long start, end; };
struct amba_device { struct device dev; struct resource res; unsigned irq[1]; unsigned periphid; };
struct amba_id { unsigned id, mask; void *data; };
struct amba_driver {
    struct { const char *name; } drv;
    const struct amba_id *id_table;
    int (*probe)(struct amba_device *, const struct amba_id *);
    void (*remove)(struct amba_device *);
};
#define module_amba_driver(driver) struct amba_driver *jnox_linux_amba_driver(void) { return &(driver); }
struct amba_driver *jnox_linux_amba_driver(void);
#define MODULE_DEVICE_TABLE(type, ids)
#define MODULE_AUTHOR(author)
#define MODULE_DESCRIPTION(desc)
#define MODULE_LICENSE(license)
#define dev_dbg(...) ((void)0)
#define dev_err(...) ((void)0)
static inline void *dev_get_drvdata(struct device *dev) { return dev->driver_data; }
static inline void amba_set_drvdata(struct amba_device *dev, void *p) { dev->dev.driver_data = p; }
static inline unsigned long resource_size(const struct resource *r) { return r->end - r->start + 1; }
struct rtc_time {
    int tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, tm_wday, tm_yday, tm_isdst;
};
struct rtc_wkalrm { unsigned char enabled, pending; struct rtc_time time; };
struct rtc_class_ops {
    int (*read_time)(struct device *, struct rtc_time *);
    int (*set_time)(struct device *, struct rtc_time *);
    int (*read_alarm)(struct device *, struct rtc_wkalrm *);
    int (*set_alarm)(struct device *, struct rtc_wkalrm *);
    int (*alarm_irq_enable)(struct device *, unsigned);
};
struct rtc_device {
    const struct rtc_class_ops *ops;
    time64_t range_min;
    timeu64_t range_max;
    unsigned long features[1];
};
static inline void clear_bit(unsigned n, unsigned long *v) { *v &= ~(1UL << n); }
static inline unsigned bin2bcd(unsigned v) { return (v / 10) * 16 + v % 10; }
static inline unsigned bcd2bin(unsigned char v) { return (v >> 4) * 10 + (v & 15); }
int amba_request_regions(struct amba_device *, const char *);
void amba_release_regions(struct amba_device *);
void *devm_kzalloc(struct device *, size_t, int);
void *devm_kmemdup(struct device *, const void *, size_t, int);
void *devm_ioremap(struct device *, unsigned long, unsigned long);
uint32_t readl(const void *);
void writel(uint32_t, void *);
struct rtc_device *devm_rtc_allocate_device(struct device *);
int devm_rtc_register_device(struct rtc_device *);
int devm_device_init_wakeup(struct device *);
int devm_pm_set_wake_irq(struct device *, int);
int request_irq(unsigned, irq_handler_t, unsigned long, const char *, void *);
void free_irq(unsigned, void *);
void rtc_update_irq(struct rtc_device *, unsigned long, unsigned long);
time64_t rtc_tm_to_time64(struct rtc_time *);
void rtc_time64_to_tm(time64_t, struct rtc_time *);
int rtc_year_days(int, int, int);
#endif
