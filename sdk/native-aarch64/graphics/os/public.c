/* Picolibc and the JNOX service ABI use different socket errno values. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>

void jnox_graphics_errno(int code)
{
    switch (code) {
    case 1: errno = EPERM; break;
    case 3: errno = ESRCH; break;
    case 11: errno = EAGAIN; break;
    case 16: errno = EBUSY; break;
    case 35: errno = EDEADLK; break;
    case 4: errno = EINTR; break;
    case 14: errno = EFAULT; break;
    case 75: errno = EOVERFLOW; break;
    case 22: errno = EINVAL; break;
    case 38: errno = ENOSYS; break;
    case 95: errno = ENOTSUP; break;
    case 2: errno = ENOENT; break;
    case 6: errno = ENXIO; break;
    case 9: errno = EBADF; break;
    case 13: errno = EACCES; break;
    case 17: errno = EEXIST; break;
    case 23: errno = ENFILE; break;
    case 24: errno = EMFILE; break;
    case 28: errno = ENOSPC; break;
    case 32: errno = EPIPE; break;
    case 36: errno = ENAMETOOLONG; break;
    case 110: errno = ETIMEDOUT; break;
    case 5: errno = EIO; break;
    case 89: errno = EDESTADDRREQ; break;
    case 90: errno = EMSGSIZE; break;
    case 91: errno = EPROTOTYPE; break;
    case 94: errno = ESOCKTNOSUPPORT; break;
    case 98: errno = EADDRINUSE; break;
    case 106: errno = EISCONN; break;
    case 107: errno = ENOTCONN; break;
    case 111: errno = ECONNREFUSED; break;
    default: errno = EIO; break;
    }
}

extern int jnox_transport_fcntl(int, int, ...);
int fcntl(int fd, int cmd, ...)
{
    // Picolibc uses 14 for F_DUPFD_CLOEXEC; the native SDK uses 1030.
    if (cmd == F_DUPFD || cmd == F_DUPFD_CLOEXEC) {
        va_list ap;
        va_start(ap, cmd);
        int minimum = va_arg(ap, int);
        va_end(ap);
        return jnox_transport_fcntl(fd, cmd == F_DUPFD ? 0 : 1030, minimum);
    }
    if (cmd == F_GETFD) return jnox_transport_fcntl(fd, 1);
    if (cmd == F_SETFD) {
        va_list ap;
        va_start(ap, cmd);
        int flags = va_arg(ap, int);
        va_end(ap);
        if (flags & ~FD_CLOEXEC) {
            errno = EINVAL;
            return -1;
        }
        return jnox_transport_fcntl(fd, 2, flags ? 1 : 0);
    }
    errno = ENOTSUP;
    return -1;
}
