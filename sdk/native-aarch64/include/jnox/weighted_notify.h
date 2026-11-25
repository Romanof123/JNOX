#ifndef JNOX_WEIGHTED_NOTIFY_H
#define JNOX_WEIGHTED_NOTIFY_H

#include <stdint.h>

/* Available only in a weighted package built with --weighted-notify.
   Returns zero until the requested event count is reached, then one. */
static inline uint64_t jnox_weighted_notification(void)
{
    return *(const volatile uint64_t *)(uintptr_t)0x01200000u;
}

#endif
