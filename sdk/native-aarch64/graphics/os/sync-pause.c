#include <errno.h>
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

int jnox_graphics_pause_waiter(void)
{
    struct { long sec, ns; } now;
    long r = jnox_ipc_trap(JNOX_CALL_CLOCK_GETTIME, 1, (uintptr_t)&now, 0).status;
    if (r) return EIO;
    now.ns += 1000000;
    if (now.ns >= 1000000000) {
        now.ns -= 1000000000;
        ++now.sec;
    }
    r = jnox_ipc_trap(JNOX_CALL_CLOCK_NANOSLEEP, 1, 1, (uintptr_t)&now).status;
    return r == 0 || r == -4 ? 0 : EIO;
}

