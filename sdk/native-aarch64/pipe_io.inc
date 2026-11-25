/* Compiled only into the opt-in native SDK io.c. The package, rather than
 * the Unix creation mode, declares the namespace and access grants. */
#include <sys/stat.h>

enum { PIPE_LAST_FD = 10, FIFO_NAME_MAX = 31 };
static unsigned char pipe_closed[PIPE_LAST_FD + 1];

static long pipe_result(long result) {
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    return result;
}
static long fifo_name_length(const char *s) {
    if (!s) return -EFAULT;
    for (unsigned i = 0; i <= FIFO_NAME_MAX; ++i) {
        if (!s[i]) return i ? (long)i : -ENOENT;
    }
    return -ENAMETOOLONG;
}

/* Check ownership before I/O. A live read end rejects a write with EBADF,
 * which must not cause the same bytes to be written to a legacy descriptor. */
static long pipe_owned(int fd) {
    if (fd < 0 || fd > PIPE_LAST_FD) return 0;
    long status = raw_service(49, fd, F_GETFD, 0, 0);
    if (status >= 0) return 1;
    if (status == -ENOSYS) return 0;
    if (status != -EBADF || pipe_closed[fd]) return status;
    return 0;
}

int pipe2(int fds[2], int flags) {
    if (!fds) return pipe_result(-EFAULT);
    register long x0 __asm__("x0") = flags;
    register long x1 __asm__("x1");
    register long x2 __asm__("x2");
    register long x8 __asm__("x8") = 41;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "=r"(x1), "=r"(x2)
                     : "r"(x8) : "cc", "memory");
    if (x0 < 0) return pipe_result(x0);
    fds[0] = (int)x1;
    fds[1] = (int)x2;
    return 0;
}
int pipe(int fds[2]) { return pipe2(fds, 0); }
int dup2(int fd, int target) {
    if (target < 0) return pipe_result(-EBADF);
    return service(42, fd, target, 0);
}
int dup(int fd) { return service(42, fd, -1, 0); }
int fcntl(int fd, int cmd, ...) {
    int arg = 0;
    if (cmd == F_SETFD || cmd == F_SETFL) {
        __builtin_va_list a;
        __builtin_va_start(a, cmd);
        arg = __builtin_va_arg(a, int);
        __builtin_va_end(a);
    }
    return service(49, fd, cmd, arg);
}
static int close_pipe_fd(int fd) {
    long r = raw_service(48, fd, 0, 0, 0);
    if (r == 0 && fd >= 0 && fd <= PIPE_LAST_FD) pipe_closed[fd] = 1;
    return (int)pipe_result(r);
}

/* Counted Rust paths do not require a terminator or ordinary-file fallback. */
long jnox_pipe_name_op(int op, const unsigned char *path, size_t len, int flags) {
    if (op != 43 && op != 44 && op != 45) return -EINVAL;
    return raw_service(op, (long)path, (long)len, flags, 0);
}
static int named_fifo(int op, const char *path) {
    long n = fifo_name_length(path);
    return (int)pipe_result(n < 0 ? n : jnox_pipe_name_op(op, (const unsigned char *)path, n, 0));
}
int mkfifo(const char *path, mode_t mode) {
    (void)mode;
    return named_fifo(43, path);
}
int unlink(const char *path) { return named_fifo(44, path); }
