#include <errno.h>
#include <fcntl.h>
#include <jnox/native_abi.h>
#include <sched.h>
#include <unistd.h>
#include <time.h>

_Static_assert(sizeof(void *) == 8 && sizeof(long) == 8 && sizeof(int) == 4,
               "native ABI v1 requires AArch64 LP64");

int *__jnox_errno_location(void)
{
    static int initial_thread_errno;
    return &initial_thread_errno;
}

/* EL1 preserves all user GPRs except the x0 result across this exception. */
static long raw_service(long id, long first, long second, long third, long fourth)
{
    register long x0 __asm__("x0") = first;
    register long x1 __asm__("x1") = second;
    register long x2 __asm__("x2") = third;
    register long x3 __asm__("x3") = fourth;
    register long x8 __asm__("x8") = id;
    __asm__ volatile("svc %c[trap]" : "+r"(x0)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x8), [trap] "i"(JNOX_ABI_SVC)
                     : "memory", "cc");
    return x0;
}

static long service(long id, long first, long second, long third)
{
    long x0 = raw_service(id, first, second, third, 0);
    if (x0 < 0) {
        /* The supervisor ABI limits errno to 1..4095. */
        errno = (int)-x0;
        return -1;
    }
    return x0;
}

int sched_yield(void)
{
    return (int)service(JNOX_CALL_SCHED_YIELD, 0, 0, 0);
}

int open(const char *path, int flags, ...)
{
    if (flags != O_RDONLY) {
        errno = EINVAL;
        return -1;
    }
    return (int)service(JNOX_CALL_OPEN, (long)path, flags, 0);
}

ssize_t read(int fd, void *buffer, size_t count)
{
    return service(JNOX_CALL_READ, fd, (long)buffer, (long)count);
}

ssize_t write(int fd, const void *buffer, size_t count)
{
    return service(JNOX_CALL_WRITE, fd, (long)buffer, (long)count);
}

int close(int fd)
{
    return (int)service(JNOX_CALL_CLOSE, fd, 0, 0);
}

_Static_assert(sizeof(struct timespec) == 16 &&
               __builtin_offsetof(struct timespec, tv_nsec) == 8,
               "native timespec requires two signed LP64 fields");

int clock_gettime(clockid_t clock, struct timespec *result)
{
    return (int)service(JNOX_CALL_CLOCK_GETTIME, clock, (long)result, 0);
}

int clock_nanosleep(clockid_t clock, int flags, const struct timespec *request,
                    struct timespec *remaining)
{
    (void)remaining;
    return (int)-raw_service(JNOX_CALL_CLOCK_NANOSLEEP, clock, flags, (long)request, 0);
}
