#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <unistd.h>
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

static unsigned int account_id(unsigned int field)
{
    long value = jnox_ipc_trap(JNOX_CALL_SERVICE_ACCOUNT, field, 0, 0).status;
    /* The four POSIX getters have no error return. A mismatched kernel must
     * never turn into an invented root identity or an equal pair of -1s. */
    if (value < 1 || value > 65534) _exit(126);
    return (unsigned int)value;
}

uid_t getuid(void)  { return (uid_t)account_id(0); }
uid_t geteuid(void) { return getuid(); }
gid_t getgid(void)  { return (gid_t)account_id(1); }
gid_t getegid(void) { return getgid(); }

int seteuid(uid_t uid)
{
    long value = jnox_ipc_trap(JNOX_CALL_SERVICE_ACCOUNT, 2, uid, 0).status;
    if (value == 0) return 0;
    switch (value) {
    case -1: errno = EPERM; break;
    case -22: errno = EINVAL; break;
    case -38: errno = ENOSYS; break;
    default: errno = EIO; break;
    }
    return -1;
}
