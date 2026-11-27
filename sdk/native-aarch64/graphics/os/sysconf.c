#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <unistd.h>
#include "jnox/shared_fd_abi.h"

long sysconf(int name)
{
    if (name == _SC_OPEN_MAX)
        return JNOX_SHM_FD_BASE + JNOX_SHM_FD_COUNT;
    errno = EINVAL;
    return -1;
}
