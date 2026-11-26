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
#endif
