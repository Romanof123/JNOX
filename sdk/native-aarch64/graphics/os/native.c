#include <stddef.h>
/* Built against the private SDK ABI. Only native errno numbers reach this TU. */
#include <errno.h>
#include <time.h>
#include <fcntl.h>
#include <jnox/ipc.h>
#include <jnox/native_abi.h>

void jnox_graphics_errno(int);
static int private_errno;
int *__jnox_errno_location(void) { return &private_errno; }

static long result(long value)
{
    if (value >= 0) return value;
    private_errno = -value;
    jnox_graphics_errno(private_errno);
    return -1;
}

int __real_close(int fd)
{
    return result(jnox_ipc_trap(JNOX_CALL_CLOSE, fd, 0, 0).status);
}
long __real_read(int fd, void *buf, size_t n)
{
    return result(jnox_ipc_trap(JNOX_CALL_READ, fd, (uintptr_t)buf, n).status);
}
long __real_write(int fd, const void *buf, size_t n)
{
    return result(jnox_ipc_trap(JNOX_CALL_WRITE, fd, (uintptr_t)buf, n).status);
}
int __real_fcntl(int fd, int cmd, ...)
{
    (void)fd;
    (void)cmd;
    return result(-ENOTSUP);
}
int jnox_poll_clock(int clock, struct timespec *ts)
{
    return result(jnox_ipc_trap(JNOX_CALL_CLOCK_GETTIME, clock, (uintptr_t)ts, 0).status);
}
int jnox_poll_sleep(int clock, int flags, const struct timespec *ts, struct timespec *rest)
{
    (void)rest;
    return -jnox_ipc_trap(JNOX_CALL_CLOCK_NANOSLEEP, clock, flags, (uintptr_t)ts).status;
}

int jnox_monotonic_now(long *seconds, long *nanoseconds)
{
    struct timespec now;
    if (jnox_poll_clock(CLOCK_MONOTONIC, &now) < 0) return -1;
    *seconds = now.tv_sec;
    *nanoseconds = now.tv_nsec;
    return 0;
}

int jnox_monotonic_getres(void *output)
{
    return result(jnox_ipc_trap(JNOX_CALL_CLOCK_GETRES, CLOCK_MONOTONIC, (uintptr_t)output, 0).status);
}

int jnox_gettimeofday(void *output, void *timezone)
{
    return result(jnox_ipc_trap(JNOX_CALL_GETTIMEOFDAY, (uintptr_t)output, (uintptr_t)timezone, 0).status);
}
