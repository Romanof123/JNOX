#ifndef JNOX_RAW_ETHERNET_H
#define JNOX_RAW_ETHERNET_H
#include <jnox/ipc.h>
#include <stddef.h>

enum { JNOX_ETHERNET_MAX_FRAME = 1514 };

/* Complete frame length on success, negative errno otherwise. */
static inline long jnox_ethernet_receive(unsigned char buffer[static JNOX_ETHERNET_MAX_FRAME])
{
    return jnox_ipc_trap(38, (uint64_t)buffer, JNOX_ETHERNET_MAX_FRAME, 0).status;
}

static inline long jnox_ethernet_transmit(const unsigned char *frame, size_t len)
{
    return jnox_ipc_trap(39, (uint64_t)frame, len, 0).status;
}

/* Only the packaged standby can claim a successfully retired NIC. */
static inline long jnox_ethernet_claim(void)
{
    return jnox_ipc_trap(76, 0, 0, 0).status;
}
#endif
