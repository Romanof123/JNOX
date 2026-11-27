#define _GNU_SOURCE
#include "jnox/xauthority_stream.h"
#include <errno.h>
#include <string.h>

void jnox_xauthority_wipe(void *buf, size_t size)
{
    volatile unsigned char *p = buf;
    while (size--) *p++ = 0;
}

static int resource_errno(long status)
{
    switch (status) {
    case -2: return ENOENT;
    case -13: return EACCES;
    case -14: return EFAULT;
    case -22: return EINVAL;
    case -34: return ERANGE;
    case -38: return ENOSYS;
    default: return EIO;
    }
}

FILE *jnox_xauthority_open(const char *path, const char *mode,
                           unsigned char storage[JNOX_XAUTHORITY_MAX],
                           size_t *length)
{
    if (!path || !mode || !storage || !length) { errno = EINVAL; return NULL; }
    *length = 0;
    if (strcmp(path, JNOX_XAUTHORITY_PATH)) { errno = ENOENT; return NULL; }
    if (strcmp(mode, "r") && strcmp(mode, "rb")) { errno = EROFS; return NULL; }
    long n = jnox_xauthority_size();
    if (n <= 0 || n > JNOX_XAUTHORITY_MAX) {
        errno = n < 0 ? resource_errno(n) : EINVAL;
        return NULL;
    }
    long got = jnox_xauthority_read(storage, (size_t)n);
    if (got != n) {
        jnox_xauthority_wipe(storage, (size_t)n);
        errno = got < 0 ? resource_errno(got) : EIO;
        return NULL;
    }
    FILE *f = fmemopen(storage, (size_t)n, "r");
    if (f) *length = (size_t)n;
    else jnox_xauthority_wipe(storage, (size_t)n);
    return f;
}
