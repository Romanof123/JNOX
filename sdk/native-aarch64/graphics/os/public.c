/* Picolibc and the JNOX service ABI use different socket errno values. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stddef.h>
#include <sys/time.h>

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
    case 61: errno = ENODATA; break;
    case 95: errno = ENOTSUP; break;
    case 2: errno = ENOENT; break;
    case 6: errno = ENXIO; break;
    case 9: errno = EBADF; break;
    case 13: errno = EACCES; break;
    case 17: errno = EEXIST; break;
    case 23: errno = ENFILE; break;
    case 24: errno = EMFILE; break;
    case 25: errno = ENOTTY; break;
    case 21: errno = EISDIR; break;
    case 20: errno = ENOTDIR; break;
    case 39: errno = ENOTEMPTY; break;
    case 30: errno = EROFS; break;
    case 27: errno = EFBIG; break;
    case 28: errno = ENOSPC; break;
    case 29: errno = ESPIPE; break;
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

#include <stdio.h>
#include <unistd.h>
extern long jnox_file_seek(int, long, int);
_Static_assert(sizeof(off_t) == sizeof(long), "graphics file offsets require LP64");
_Static_assert(SEEK_SET == 0 && SEEK_CUR == 1 && SEEK_END == 2, "graphics seek selectors");
off_t lseek(int fd, off_t offset, int whence)
{
    return jnox_file_seek(fd, offset, whence);
}

extern int jnox_transport_fcntl(int, int, ...);
_Static_assert(O_RDONLY == 0 && O_WRONLY == 1 && O_RDWR == 2 && O_ACCMODE == 3,
               "native and Picolibc access mode bits must agree");
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
    /* The two libc interfaces use different values for O_NONBLOCK. */
    if (cmd == F_GETFL) {
        int flags = jnox_transport_fcntl(fd, 3);
        if (flags < 0) return -1;
        return (flags & O_ACCMODE) | ((flags & 0x800) ? O_NONBLOCK : 0);
    }
    if (cmd == F_SETFL) {
        va_list ap;
        va_start(ap, cmd);
        int flags = va_arg(ap, int);
        va_end(ap);
        if (flags & ~(O_ACCMODE | O_NONBLOCK)) {
            errno = EINVAL;
            return -1;
        }
        return jnox_transport_fcntl(fd, 4, (flags & O_NONBLOCK) ? 0x800 : 0);
    }
    if (cmd == F_GET_SEALS) return jnox_transport_fcntl(fd, F_GET_SEALS);
    if (cmd == F_ADD_SEALS) {
        va_list ap;
        va_start(ap, cmd);
        int seals = va_arg(ap, int);
        va_end(ap);
        return jnox_transport_fcntl(fd, F_ADD_SEALS, seals);
    }
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

#include <time.h>
extern int jnox_monotonic_now(long *, long *);
extern int jnox_monotonic_getres(void *);
extern int jnox_gettimeofday(void *, void *);

_Static_assert(sizeof(struct timeval) == 16 && sizeof(suseconds_t) == 8 &&
               offsetof(struct timeval, tv_usec) == 8, "native timeval layout");

int gettimeofday(struct timeval *value, void *timezone)
{
    return jnox_gettimeofday(value, timezone);
}

_Static_assert(sizeof(struct timespec) == 16 && sizeof(time_t) == 8 &&
               offsetof(struct timespec, tv_nsec) == 8 && sizeof(long) == 8,
               "native clock result uses signed LP64 seconds and nanoseconds");

int clock_getres(clockid_t clock, struct timespec *result)
{
    if (clock != CLOCK_MONOTONIC) {
        errno = EINVAL;
        return -1;
    }
    return jnox_monotonic_getres(result);
}

int clock_gettime(clockid_t clock, struct timespec *result)
{
    if (clock != CLOCK_MONOTONIC) {
        errno = EINVAL;
        return -1;
    }
    if (!result) {
        errno = EFAULT;
        return -1;
    }
    long seconds, nanoseconds;
    if (jnox_monotonic_now(&seconds, &nanoseconds) < 0) return -1;
    time_t public_seconds = (time_t)seconds;
    if (seconds < 0 || (long)public_seconds != seconds ||
        nanoseconds < 0 || nanoseconds >= 1000000000L) {
        errno = EOVERFLOW;
        return -1;
    }
    result->tv_sec = public_seconds;
    result->tv_nsec = nanoseconds;
    return 0;
}
