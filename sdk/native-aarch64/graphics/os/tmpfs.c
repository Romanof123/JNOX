#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include "jnox/ipc.h"
#include "jnox/tmpfs_fd.h"

static unsigned int mask = 022;
void jnox_graphics_errno(int code);

static long complete(long r)
{
    if (r >= 0) return r;
    jnox_graphics_errno((int)-r);
    return -1;
}
#include "../../tmpfs_transport.inc"
static long path_len(const char *path)
{
    uintptr_t addr = (uintptr_t)path;
    if (!addr) return -14;
    for (size_t n = 0; n <= MAX_PATH_BYTES; ++n) {
        if (addr > UINTPTR_MAX - n) return -14;
        const char *p = (const char *)(addr+n);
        long rc = user_range(p, 1, 0);
        if (rc < 0) return rc;
        if (*p == 0) return (long)n;
    }
    return -36;
}
mode_t umask(mode_t mode)
{
    return __atomic_exchange_n(&mask, mode & 0777, __ATOMIC_RELAXED);
}
int open(const char *path, int flags, ...)
{
    switch (flags & O_ACCMODE) {
    case O_RDONLY: case O_WRONLY: case O_RDWR: break;
    default: return complete(-22);
    }
    if (flags & ~(O_ACCMODE|O_CREAT|O_EXCL|O_TRUNC|O_APPEND|O_CLOEXEC)) return complete(-95);
    unsigned wire = (flags & O_ACCMODE) == O_RDWR ? R|W : (flags & O_ACCMODE) == O_WRONLY ? W : R;
    unsigned mode = 0;
    if (flags & O_CREAT) {
        va_list ap; va_start(ap, flags); mode = va_arg(ap, unsigned int); va_end(ap);
        if (mode & ~0777U) return complete(-95);
        wire |= CREATE;
        mode &= ~__atomic_load_n(&mask, __ATOMIC_RELAXED);
    }
    if (flags & O_EXCL) wire |= EXCL;
    if (flags & O_TRUNC) wire |= TRUNC;
    if (flags & O_APPEND) wire |= APPEND;
    if (flags & O_CLOEXEC) wire |= CLOEXEC;
    long n = path_len(path);
    return complete(n < 0 ? n : exchange(OPEN, 0, wire, mode, path, n, NULL, 0));
}
int unlink(const char *path)
{
    long n = path_len(path);
    return complete(n < 0 ? n : exchange(UNLINK, 0, 0, 0, path, n, NULL, 0));
}
int mkdir(const char *path, mode_t mode)
{
    if (mode & ~01777U) return complete(-95);
    long n = path_len(path);
    mode &= ~__atomic_load_n(&mask, __ATOMIC_RELAXED);
    return complete(n < 0 ? n : exchange(MKDIR, 0, 0, mode, path, n, NULL, 0));
}
static int two_paths(unsigned op, const char *first, const char *second)
{
    long n = path_len(first);
    if (n < 0) return complete(n);
    long m = path_len(second);
    if (m < 0) return complete(m);
    unsigned char pair[MAX_REQUEST];
    memcpy(pair, first, (size_t)n); pair[n] = 0;
    memcpy(pair+n+1, second, (size_t)m);
    return complete(exchange(op, 0, 0, 0, pair, (size_t)(n+1+m), NULL, 0));
}
int rename(const char *first, const char *second) { return two_paths(RENAME, first, second); }
int link(const char *first, const char *second) { return two_paths(LINK, first, second); }
int chmod(const char *path, mode_t mode)
{
    if (mode & ~01777U) return complete(-95);
    long n = path_len(path);
    return complete(n < 0 ? n : exchange(CHMOD, 0, 0, mode, path, n, NULL, 0));
}
static int descriptor_op(unsigned op, int fd, uint32_t arg)
{
    if (!jnox_tmp_fd(fd)) return jnox_graphics_unsupported_fd(fd);
    return complete(exchange(op, fd, 0, arg, NULL, 0, NULL, 0));
}
int fchmod(int fd, mode_t mode)
{
    return descriptor_op(FCHMOD, fd, mode);
}
int fsync(int fd)
{
    return descriptor_op(TMPFS_FSYNC, fd, 0);
}
long jnox_tmp_seek(int fd, long offset, int whence)
{
    if (whence < SEEK_SET || whence > SEEK_END) return complete(-22);
    return complete(exchange(SEEK, fd, whence, 0, &offset, 8, NULL, 0));
}
long jnox_tmp_read(int fd, void *buffer, size_t size)
{
    size_t n = size < MAX_PAYLOAD ? size : MAX_PAYLOAD;
    return complete(exchange(READ, fd, 0, n, NULL, 0, buffer, n));
}
long jnox_tmp_write(int fd, const void *buffer, size_t size)
{
    size_t n = size < MAX_PAYLOAD ? size : MAX_PAYLOAD;
    return complete(exchange(WRITE, fd, 0, 0, buffer, n, NULL, 0));
}
int jnox_tmp_close(int fd) { return complete(exchange(CLOSE, fd, 0, 0, NULL, 0, NULL, 0)); }
int jnox_tmp_dup2(int fd, int target)
{
    if (!jnox_tmp_fd(target)) return complete(target < 0 ? -9 : -95);
    return complete(exchange(DUP, fd, 2, target, NULL, 0, NULL, 0));
}
int jnox_tmp_fcntl(int fd, int cmd, int arg)
{
    /* Comes from the native transport after Picolibc command translation. */
    if ((cmd == 0 || cmd == 1030) && arg >= 0)
        return complete(exchange(DUP, fd, cmd == 1030, arg, NULL, 0, NULL, 0));
    if (cmd == 1 || (cmd == 2 && (arg == 0 || arg == 1)))
        return complete(exchange(FLAGS, fd, cmd == 2, cmd == 2 ? arg : 0, NULL, 0, NULL, 0));
    return complete(cmd == 0 || cmd == 1030 || cmd == 2 ? -22 : -95);
}
static long metadata(int op, int fd, const char *path, struct stat *result)
{
    long ok = user_range(result, sizeof(*result), 1);
    if (ok < 0) return ok;
    long n = op == FSTAT ? 0 : path_len(path);
    if (n < 0) return n;
    unsigned char wire[STAT_BYTES];
    long rc = exchange(op, fd, 0, 0, path, n, wire, sizeof wire);
    if (rc < 0) return rc;
    if (rc > 1) return -5;
    uint32_t ino, size; uint16_t mode, links;
    memcpy(&ino, wire, 4); memcpy(&mode, wire+4, 2); memcpy(&links, wire+6, 2); memcpy(&size, wire+8, 4);
    struct stat s = {0};
    s.st_dev = 1; s.st_mode = mode; s.st_nlink = links; s.st_size = size; s.st_ino = ino;
    if ((uint32_t)s.st_ino != ino) return -75;
    s.st_uid = rc == 1 ? 0 : getuid(); s.st_gid = rc == 1 ? 0 : getgid(); s.st_blksize = 64;
    s.st_blocks = S_ISREG(mode) ? 2 : 0; /* Fixed 1024-byte data slot, in 512-byte units. */
    struct timespec *times[] = {&s.st_atim, &s.st_mtim, &s.st_ctim};
    for (unsigned i = 0; i < 3; ++i) {
        int64_t sec; uint32_t micros;
        memcpy(&sec, wire+12+i*12, 8); memcpy(&micros, wire+20+i*12, 4);
        times[i]->tv_sec = sec; times[i]->tv_nsec = (long)micros * 1000;
        if ((int64_t)times[i]->tv_sec != sec) return -75;
    }
    *result = s; return 0;
}
int fstat(int fd, struct stat *s)
{
    if (!jnox_tmp_fd(fd)) return jnox_graphics_unsupported_fd(fd);
    return complete(metadata(FSTAT, fd, NULL, s));
}
int stat(const char *path, struct stat *s) { return complete(metadata(STAT, 0, path, s)); }
int lstat(const char *path, struct stat *s) { return stat(path, s); }
