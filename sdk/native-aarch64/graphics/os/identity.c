#include <errno.h>
#include <string.h>
#include <unistd.h>
#include "sys/utsname.h"
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

_Static_assert(sizeof(struct utsname) == 325, "native uname record layout");
int uname(struct utsname *out)
{
    long r = jnox_ipc_trap(JNOX_CALL_UNAME, (uintptr_t)out, sizeof(*out), 0).status;
    if (r == 0) return 0;
    switch (r) {
    case -61: errno = ENODATA; break;
    case -14: errno = EFAULT; break;
    case -22: errno = EINVAL; break;
    case -38: errno = ENOSYS; break;
    case -95: errno = ENOTSUP; break;
    default: errno = EIO; break;
    }
    return -1;
}

int gethostname(char *name, size_t size)
{
    struct utsname u;
    if (uname(&u)) return -1;
    size_t required = strlen(u.nodename) + 1;
    if (size < required) { errno = ENAMETOOLONG; return -1; }
    if (!name) { errno = EFAULT; return -1; }
    memcpy(name, u.nodename, required);
    return 0;
}
