#ifndef JNOX_DISPLAY_H
#define JNOX_DISPLAY_H
#include <stdint.h>

enum { JNOX_DISPLAY_WIDTH = 320, JNOX_DISPLAY_HEIGHT = 240 };

/* Owner-only native virt ABI. The result is a value or a negative errno. */
static inline long jnox_display_call(unsigned op, unsigned arg, const uint32_t *row)
{
    register unsigned long x0 __asm__("x0") = arg;
    register const uint32_t *x1 __asm__("x1") = row;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1)
                     : "r"(x8) : "x2", "x3", "memory", "cc");
    return (long)x0;
}

static inline long jnox_display_init(void)
{
    return jnox_display_call(72, 0, 0);
}

/* Replace one complete row of XRGB8888 pixels (red in bits 16..23). */
static inline long jnox_display_write_row(unsigned y, const uint32_t row[JNOX_DISPLAY_WIDTH])
{
    return jnox_display_call(73, y, row);
}

/* One PL011 input byte, or -EAGAIN if the receiver is empty. */
static inline long jnox_display_poll_key(void)
{
    return jnox_display_call(74, 0, 0);
}

enum jnox_rx_metric {
    JNOX_RX_IRQ_ENTRIES = 0,
    JNOX_RX_BYTES = 1,       /* valid hardware bytes, including software ring drops */
    JNOX_RX_DROPS = 2,
    JNOX_RX_ERROR_WORDS = 3, /* hardware receive errors */
    JNOX_RX_DEPTH = 4,
    JNOX_RX_CAPACITY = 5,
    JNOX_RX_IDLE_IRQ_ENTRIES = 6
};

/* Sample one metric since boot. IRQ profile owner only; polled profiles return -ENOSYS. */
static inline long jnox_display_rx_stat(enum jnox_rx_metric metric)
{
    return jnox_display_call(75, (unsigned)metric, 0);
}
#endif
