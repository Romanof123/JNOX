#ifndef JNOX_BOOT_RESOURCE_H
#define JNOX_BOOT_RESOURCE_H

#include <stddef.h>
#include <stdint.h>
#include <jnox/ipc.h>

#define JNOX_CALL_XAUTHORITY 199
#define JNOX_XAUTHORITY_PATH "/run/Xauthority"
#define JNOX_XAUTHORITY_MAX 256

/* Both operations return the record length or a negative native errno. */
static inline long jnox_xauthority_size(void)
{
    return jnox_ipc_trap(JNOX_CALL_XAUTHORITY, 0, 0, 0).status;
}

static inline long jnox_xauthority_read(void *destination, size_t capacity)
{
    return jnox_ipc_trap(JNOX_CALL_XAUTHORITY, 1, (uintptr_t)destination, capacity).status;
}
#endif
