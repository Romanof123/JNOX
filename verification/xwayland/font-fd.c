/* JNOX ordinary descriptor operations needed by upstream zlib's gzclose.
 * No host or Linux syscall convention enters the normal target link. */
#include <errno.h>
#include <stdint.h>
#include <unistd.h>
#include "jnox/ipc.h"
#include "jnox/native_abi.h"
static long fd_result(long value)
{
    if (value >= 0) return value;
    switch (-value) {
    case 1: errno = EPERM; break;
    case 4: errno = EINTR; break;
    case 5: errno = EIO; break;
    case 9: errno = EBADF; break;
    case 11: errno = EAGAIN; break;
    case 13: errno = EACCES; break;
    case 14: errno = EFAULT; break;
    case 22: errno = EINVAL; break;
    case 28: errno = ENOSPC; break;
    case 32: errno = EPIPE; break;
    case 38: errno = ENOSYS; break;
    case 95: errno = ENOTSUP; break;
    default: errno = EIO; break;
    }
    return -1;
}
int close(int fd)
{
    return fd_result(jnox_ipc_trap(JNOX_CALL_CLOSE, fd, 0, 0).status);
}
ssize_t write(int fd, const void *buf, size_t n)
{
    return fd_result(jnox_ipc_trap(JNOX_CALL_WRITE, fd, (uintptr_t)buf, n).status);
}
ssize_t read(int fd, void *buf, size_t n)
{
    return fd_result(jnox_ipc_trap(JNOX_CALL_READ, fd, (uintptr_t)buf, n).status);
}
