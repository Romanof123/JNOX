/* Counted native Rust FFI. Return signed service results directly, without libc. */
#include <stddef.h>
#include <stdint.h>
#include "jnox/ipc.h"
#include "tmpfs_transport.inc"

long jnox_tmpfs_native_open(const unsigned char *path, size_t len, unsigned how)
{
    unsigned flags;
    switch (how) {
    case 0: flags = R; break;
    case 1: flags = R | W; break;
    case 2: flags = R | W | CREATE | EXCL; break;
    default: return -22;
    }
    if (len > MAX_PATH_BYTES) return -36;
    return exchange(OPEN, 0, flags, how == 2 ? 0600 : 0, path, len, NULL, 0);
}

long jnox_tmpfs_native_unlink(const unsigned char *path, size_t len)
{
    if (len > MAX_PATH_BYTES) return -36;
    return exchange(UNLINK, 0, 0, 0, path, len, NULL, 0);
}

long jnox_tmpfs_native_close(int fd)
{
    return exchange(CLOSE, fd, 0, 0, NULL, 0, NULL, 0);
}

long jnox_tmpfs_native_read(int fd, void *buf, size_t len)
{
    size_t n = len < MAX_PAYLOAD ? len : MAX_PAYLOAD;
    return exchange(READ, fd, 0, n, NULL, 0, buf, n);
}

long jnox_tmpfs_native_write(int fd, const void *buf, size_t len)
{
    size_t n = len < MAX_PAYLOAD ? len : MAX_PAYLOAD;
    return exchange(WRITE, fd, 0, 0, buf, n, NULL, 0);
}

long jnox_tmpfs_native_seek(int fd, unsigned whence, int64_t offset)
{
    return exchange(SEEK, fd, whence, 0, &offset, sizeof(offset), NULL, 0);
}
