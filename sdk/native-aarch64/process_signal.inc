#include <signal.h>
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

int kill(pid_t pid, int sig)
{
    long result = jnox_ipc_trap(JNOX_CALL_PROCESS_SIGNAL, (uint64_t)(int64_t)pid,
                                (uint64_t)(int64_t)sig, 0).status;
    if (!result) return 0;
    switch (result) {
    case -1: errno = EPERM; break;
    case -3: errno = ESRCH; break;
    case -22: errno = EINVAL; break;
    case -38: errno = ENOSYS; break;
    case -95: errno = ENOTSUP; break;
    default: errno = EIO; break;
    }
    return -1;
}
