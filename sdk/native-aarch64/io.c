#include <errno.h>
#include <fcntl.h>
#include <jnox/native_abi.h>
#include <jnox/clock_pair.h>
#include <jnox/deadline.h>
#include <jnox/periodic.h>
#include <pthread.h>
#include <sched.h>
#include <unistd.h>
#include <time.h>

_Static_assert(sizeof(void *) == 8 && sizeof(long) == 8 && sizeof(int) == 4,
               "native ABI v1 requires AArch64 LP64");

int *__jnox_errno_location(void)
{
    static int initial_thread_errno;
    int *cell;
    __asm__ volatile("mrs %0, tpidrro_el0" : "=r"(cell));
    return cell ? cell : &initial_thread_errno;
}

struct service_result {
    long status;
    uint64_t value;
};

static struct service_result raw_service_pair(long id, long first, long second, long third, long fourth, long fifth)
{
    register long x0 __asm__("x0") = first;
    register long x1 __asm__("x1") = second;
    register long x2 __asm__("x2") = third;
    register long x3 __asm__("x3") = fourth;
    register long x4 __asm__("x4") = fifth;
    register long x8 __asm__("x8") = id;
    __asm__ volatile("svc %c[trap]" : "+r"(x0), "+r"(x1)
                     : "r"(x2), "r"(x3), "r"(x4), "r"(x8), [trap] "i"(JNOX_ABI_SVC)
                     : "memory", "cc");
    return (struct service_result){x0, (uint64_t)x1};
}

static long raw_service(long id, long first, long second, long third, long fourth)
{
    return raw_service_pair(id, first, second, third, fourth, 0).status;
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

int jnox_deadline_service_wait_next(void)
{
    return (int)service(JNOX_CALL_DEADLINE_SERVICE_WAIT_NEXT, 0, 0, 0);
}

int jnox_periodic_wait_next(void)
{
    return (int)service(JNOX_CALL_PERIODIC_WAIT_NEXT, 0, 0, 0);
}

int sched_yield(void)
{
    return (int)service(JNOX_CALL_SCHED_YIELD, 0, 0, 0);
}

#ifdef JNOX_SDK_PIPES
#include "pipe_io.inc"
#endif
#ifdef JNOX_SDK_NATIVE_SHELL
#include "native_shell_io.inc"
#endif

int open(const char *path, int flags, ...)
{
#ifdef JNOX_SDK_PIPES
    long n = fifo_name_length(path);
    if (n == -EFAULT) return pipe_result(n);
    if (n > 0) {
        long fd = raw_service(45, (long)path, n, flags, 0);
        if (fd >= 0) return (int)fd;
        if (fd != -ENOENT && fd != -ENOSYS) return pipe_result(fd);
    }
#endif
#ifdef JNOX_SDK_NATIVE_SHELL
    return (int)pipe_result(boot_file_open(path, flags));
#else
    if (flags != O_RDONLY) {
        errno = EINVAL;
        return -1;
    }
#ifdef JNOX_SDK_PIPES
    int ordinary = (int)service(JNOX_CALL_OPEN, (long)path, flags, 0);
    if (ordinary >= 0 && ordinary <= PIPE_LAST_FD)
        pipe_closed[ordinary] = 0;
    return ordinary;
#else
    return (int)service(JNOX_CALL_OPEN, (long)path, flags, 0);
#endif
#endif
}

ssize_t read(int fd, void *buffer, size_t count)
{
#ifdef JNOX_SDK_PIPES
    long owned = pipe_owned(fd);
    if (owned < 0) return pipe_result(owned);
    if (owned) return service(46, fd, (long)buffer, (long)count);
#endif
#ifdef JNOX_SDK_NATIVE_SHELL
    if (boot_fd(fd)) return pipe_result(boot_file_read(fd, buffer, count));
#endif
    return service(JNOX_CALL_READ, fd, (long)buffer, (long)count);
}

ssize_t write(int fd, const void *buffer, size_t count)
{
#ifdef JNOX_SDK_PIPES
    long owned = pipe_owned(fd);
    if (owned < 0) return pipe_result(owned);
    if (owned) return service(47, fd, (long)buffer, (long)count);
#endif
#ifdef JNOX_SDK_NATIVE_SHELL
    if (boot_fd(fd)) return pipe_result(-EBADF);
#endif
    return service(JNOX_CALL_WRITE, fd, (long)buffer, (long)count);
}

int close(int fd)
{
#ifdef JNOX_SDK_PIPES
    long owned = pipe_owned(fd);
    if (owned < 0) return pipe_result(owned);
    if (owned) return close_pipe_fd(fd);
#endif
#ifdef JNOX_SDK_NATIVE_SHELL
    if (boot_fd(fd)) return (int)pipe_result(boot_file_close(fd));
#endif
    return (int)service(JNOX_CALL_CLOSE, fd, 0, 0);
}

_Static_assert(sizeof(struct timespec) == 16 &&
               __builtin_offsetof(struct timespec, tv_nsec) == 8,
               "native timespec requires two signed LP64 fields");

int clock_gettime(clockid_t clock, struct timespec *result)
{
    return (int)service(JNOX_CALL_CLOCK_GETTIME, clock, (long)result, 0);
}

/* All cache state is in the application's private, pristine-on-restart BSS. */
static unsigned pair_discovery;
static unsigned pair_identity;

static int ticks_to_ns(uint64_t ticks, uint64_t hz, uint64_t *ns)
{
    /* CNTFRQ is a 32-bit architectural field. Splitting at whole seconds
     * computes the wide product/division exactly without a runtime helper. */
    uint64_t seconds = ticks / hz;
    uint64_t fraction = (ticks % hz) * 1000000000ULL / hz;
    if (seconds > (UINT64_MAX - fraction) / 1000000000ULL)
        return EOVERFLOW;
    *ns = seconds * 1000000000ULL + fraction;
    return 0;
}

static int syscall_ns(clockid_t id, uint64_t *ns)
{
    struct timespec t;
    long rc = raw_service(JNOX_CALL_CLOCK_GETTIME, id, (long)&t, 0, 0);
    if (rc < 0)
        return (int)-rc;
    if (t.tv_sec < 0 || t.tv_nsec < 0 || t.tv_nsec >= 1000000000 ||
        (uint64_t)t.tv_sec > (UINT64_MAX - (uint64_t)t.tv_nsec) / 1000000000ULL)
        return EOVERFLOW;
    *ns = (uint64_t)t.tv_sec * 1000000000ULL + (uint64_t)t.tv_nsec;
    return 0;
}

struct jnox_clock_pair jnox_clock_sample(void)
{
    const long va = 0x01400000;
    unsigned discovery = __atomic_load_n(&pair_discovery, __ATOMIC_RELAXED);
    if (!discovery) {
        discovery = raw_service(JNOX_CALL_CLOCK_PAGE, 0, 0, 0, 0) == va ? 1 : 2;
        __atomic_store_n(&pair_discovery, discovery, __ATOMIC_RELAXED);
    }
    const uint64_t *page = (const uint64_t *)va;
    struct jnox_clock_pair pair = {0};
    for (unsigned attempt = 0; discovery == 1 && attempt < 2; ++attempt) {
        uint64_t seq = __atomic_load_n(page, __ATOMIC_ACQUIRE);
        if (!seq || (seq & 1))
            continue;
        uint64_t fields[7];
        for (unsigned i = 0; i < 7; ++i)
            fields[i] = __atomic_load_n(page + 1 + i, __ATOMIC_RELAXED);
        uint64_t now;
        __asm__ volatile("isb\n\tmrs %0,cntpct_el0\n\tdmb ishld" : "=r"(now) :: "memory");
        if (__atomic_load_n(page, __ATOMIC_ACQUIRE) != seq)
            continue;
        uint64_t hz = fields[3], origin = fields[4], settled = fields[5], ret = fields[6];
        if (fields[0] != 1 || fields[1] >= 4 || fields[2] >= 16 ||
            !hz || hz > 0xffffffffULL || !ret || now < ret || now < origin)
            continue;
        unsigned id = 1 + (unsigned)(fields[1] * 16 + fields[2]);
        if (pair_identity && pair_identity != id)
            continue;
        pair_identity = id;
        if (now - ret > UINT64_MAX - settled ||
            ticks_to_ns(now - origin, hz, &pair.monotonic_ns) ||
            ticks_to_ns(settled + (now - ret), hz, &pair.process_cpu_ns))
            return (struct jnox_clock_pair){.error = EOVERFLOW};
        pair.backend = 1;
        return pair;
    }
    int err = syscall_ns(CLOCK_PROCESS_CPUTIME_ID, &pair.process_cpu_ns);
    if (!err)
        err = syscall_ns(CLOCK_MONOTONIC, &pair.monotonic_ns);
    if (err)
        return (struct jnox_clock_pair){.error = (uint32_t)err};
    pair.backend = 2;
    return pair;
}

int clock_nanosleep(clockid_t clock, int flags, const struct timespec *request,
                    struct timespec *remaining)
{
    (void)remaining;
    return (int)-raw_service(JNOX_CALL_CLOCK_NANOSLEEP, clock, flags, (long)request, 0);
}

enum { thread_attr_tag = 0x54485244, mutex_attr_tag = 0x4d555458 };

_Static_assert(sizeof(pthread_attr_t) == 8 && _Alignof(pthread_attr_t) == 8, "thread attr ABI");
_Static_assert(sizeof(pthread_mutexattr_t) == 8 && _Alignof(pthread_mutexattr_t) == 8, "mutex attr ABI");

int pthread_attr_init(pthread_attr_t *attr)
{
    if (!attr)
        return EINVAL;
    *attr = (pthread_attr_t){.tag = thread_attr_tag};
    return 0;
}

int pthread_attr_destroy(pthread_attr_t *attr)
{
    if (!attr || attr->tag != thread_attr_tag)
        return EINVAL;
    attr->tag = 0;
    return 0;
}

int pthread_attr_setschedpolicy(pthread_attr_t *attr, int policy)
{
    if (!attr || attr->tag != thread_attr_tag)
        return EINVAL;
    if (policy != SCHED_FIFO && policy != SCHED_RR)
        return ENOTSUP;
    attr->policy = (unsigned char)policy;
    return 0;
}

int pthread_attr_setschedparam(pthread_attr_t *attr, const struct sched_param *param)
{
    if (!attr || attr->tag != thread_attr_tag || !param ||
        param->sched_priority < 1 || param->sched_priority > 31)
        return EINVAL;
    attr->priority = (unsigned char)param->sched_priority;
    return 0;
}

int pthread_attr_setinheritsched(pthread_attr_t *attr, int inherit)
{
    if (!attr || attr->tag != thread_attr_tag)
        return EINVAL;
    if (inherit != PTHREAD_EXPLICIT_SCHED)
        return ENOTSUP;
    attr->explicit_sched = 1;
    return 0;
}

int pthread_mutexattr_init(pthread_mutexattr_t *attr)
{
    if (!attr)
        return EINVAL;
    *attr = (pthread_mutexattr_t){.tag = mutex_attr_tag, .users = 15};
    return 0;
}

int pthread_mutexattr_destroy(pthread_mutexattr_t *attr)
{
    if (!attr || attr->tag != mutex_attr_tag)
        return EINVAL;
    attr->tag = 0;
    return 0;
}

int pthread_mutexattr_setprotocol(pthread_mutexattr_t *attr, int protocol)
{
    if (!attr || attr->tag != mutex_attr_tag)
        return EINVAL;
    if (protocol < PTHREAD_PRIO_NONE || protocol > PTHREAD_PRIO_PROTECT)
        return ENOTSUP;
    attr->protocol = (unsigned char)protocol;
    return 0;
}

int pthread_mutexattr_setprioceiling(pthread_mutexattr_t *attr, int ceiling)
{
    if (!attr || attr->tag != mutex_attr_tag || ceiling < 1 || ceiling > 31)
        return EINVAL;
    attr->ceiling = (unsigned char)ceiling;
    return 0;
}

int jnox_mutexattr_setusers(pthread_mutexattr_t *attr, uint32_t mask)
{
    if (!attr || attr->tag != mutex_attr_tag || mask == 0 || mask > 15)
        return EINVAL;
    attr->users = (unsigned char)mask;
    return 0;
}

extern void __jnox_thread_start(void *(*start)(void *), void *arg);

int pthread_create(pthread_t *thread, const pthread_attr_t *attr,
                   void *(*start)(void *), void *arg)
{
    if (!thread || !start)
        return EINVAL;
    unsigned policy = 0, priority = 0;
    if (attr) {
        if (attr->tag != thread_attr_tag)
            return ENOTSUP;
        if (attr->explicit_sched || attr->policy || attr->priority) {
            if (attr->explicit_sched != 1 ||
                (attr->policy != SCHED_FIFO && attr->policy != SCHED_RR) ||
                attr->priority < 1 || attr->priority > 31)
                return EINVAL;
            policy = attr->policy;
            priority = attr->priority;
        }
    }
    struct service_result reply = raw_service_pair(JNOX_CALL_THREAD_CREATE,
        (long)start, (long)arg, (long)__jnox_thread_start, policy, priority);
    if (reply.status)
        return (int)-reply.status;
    *thread = reply.value;
    return 0;
}

int pthread_join(pthread_t thread, void **result)
{
    struct service_result reply = raw_service_pair(JNOX_CALL_THREAD_JOIN, (long)thread, 0, 0, 0, 0);
    if (reply.status)
        return (int)-reply.status;
    if (result)
        *result = (void *)reply.value;
    return 0;
}

_Static_assert(sizeof(pthread_mutex_t) == 8 && _Alignof(pthread_mutex_t) == 8,
               "native mutex object ABI");

int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
    unsigned protocol = 0, users = 0, ceiling = 0;
    if (attr) {
        if (attr->tag != mutex_attr_tag)
            return ENOTSUP;
        protocol = attr->protocol;
        if (!attr->users || attr->users > 15 || protocol > PTHREAD_PRIO_PROTECT)
            return EINVAL;
        if (protocol || attr->users != 15)
            users = attr->users;
        if (protocol == PTHREAD_PRIO_PROTECT) {
            ceiling = attr->ceiling;
            if (ceiling < 1 || ceiling > 31)
                return EINVAL;
        }
    }
    /* The kernel validates the address and admitted roles before allocating. */
    struct service_result reply = raw_service_pair(JNOX_CALL_MUTEX_INIT, (long)mutex,
                                                   0, protocol, users, ceiling);
    if (reply.status)
        return (int)-reply.status;
    mutex->opaque = reply.value;
    return 0;
}

static int mutex_operation(long call, pthread_mutex_t *mutex)
{
    if (!mutex || ((unsigned long)mutex & 7))
        return EINVAL;
    return (int)-raw_service(call, (long)mutex, (long)mutex->opaque, 0, 0);
}

int pthread_mutex_destroy(pthread_mutex_t *mutex)
{
    int error = mutex_operation(JNOX_CALL_MUTEX_DESTROY, mutex);
    if (!error)
        mutex->opaque = 0;
    return error;
}

static int mutex_acquire(long call, pthread_mutex_t *mutex)
{
    int error = mutex_operation(call, mutex);
    if (!error)
        __atomic_thread_fence(__ATOMIC_ACQUIRE);
    return error;
}

int pthread_mutex_lock(pthread_mutex_t *mutex)
{
    return mutex_acquire(JNOX_CALL_MUTEX_LOCK, mutex);
}

int pthread_mutex_clocklock(pthread_mutex_t *mutex, clockid_t clock, const struct timespec *absolute)
{
    if (!mutex || ((unsigned long)mutex & 7) || clock != CLOCK_MONOTONIC)
        return EINVAL;
    /* Pass the timespec address unchanged for the kernel's checked user copy. */
    int error = (int)-raw_service(JNOX_CALL_MUTEX_CLOCKLOCK, (long)mutex,
                                  (long)mutex->opaque, clock, (long)absolute);
    if (!error)
        __atomic_thread_fence(__ATOMIC_ACQUIRE);
    return error;
}

int pthread_mutex_trylock(pthread_mutex_t *mutex)
{
    return mutex_acquire(JNOX_CALL_MUTEX_TRYLOCK, mutex);
}

int pthread_mutex_unlock(pthread_mutex_t *mutex)
{
    __atomic_thread_fence(__ATOMIC_RELEASE);
    return mutex_operation(JNOX_CALL_MUTEX_UNLOCK, mutex);
}
