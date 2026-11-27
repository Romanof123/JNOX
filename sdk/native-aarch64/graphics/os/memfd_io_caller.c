#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include "jnox/shm.h"

int main(void)
{
    const unsigned flags = MFD_CLOEXEC | MFD_ALLOW_SEALING;
    int fd = memfd_create("cursor", flags);
    int other = memfd_create("separate", flags);
    if (fd < 0 || other < 0) return 101;
    int alias = fcntl(fd, F_DUPFD, 0);
    int ro = jnox_shm_dup_readonly(fd);
    if (alias < 0 || ro < 0) return 102;

    unsigned char data[300];
    memset(data, 0x72, sizeof data);
    if (write(fd, "xy", 2) != 2 || lseek(alias, 0, SEEK_CUR) != 2 ||
        lseek(other, 0, SEEK_CUR) != 0 || read(other, data, 1) != 0) return 103;
    if (lseek(alias, 2, SEEK_END) != 4 || write(fd, "K", 1) != 1 ||
        jnox_shm_size(fd) != 5 || lseek(ro, 0, SEEK_SET) != 0) return 104;
    if (read(ro, data, 5) != 5 || memcmp(data, "xy\0\0K", 5) != 0 ||
        lseek(fd, 0, SEEK_CUR) != 5 || read(fd, data, 1) != 0) return 105;
    if (write(ro, "!", 1) != -1 || errno != EBADF ||
        lseek(ro, -6, SEEK_CUR) != -1 || errno != EINVAL ||
        lseek(fd, 0, SEEK_CUR) != 5) return 106;
    if (lseek(fd, LONG_MAX, SEEK_SET) != LONG_MAX ||
        lseek(alias, 1, SEEK_CUR) != -1 || errno != EOVERFLOW ||
        write(fd, "!", 1) != -1 || errno != EFBIG) return 107;

    if (lseek(fd, 0, SEEK_SET) != 0 || ftruncate(fd, 4) != 0) return 108;
    unsigned char *view = mmap(NULL, 4, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (view == MAP_FAILED) return 109;
    if (read(fd, view + 1, 2) != 2 || view[1] != 'x' || view[2] != 'y') return 110;
    if (write(fd, view + 1, 2) != 2 || view[2] != 'x' || view[3] != 'y') return 111;
    if (ftruncate(fd, 2) != -1 || errno != EBUSY || jnox_shm_size(fd) != 4 ||
        memcmp(view, "xxxy", 4) != 0) return 120;
    if (write(fd, "!", 1) != -1 || errno != EBUSY || munmap(view, 4) != 0) return 112;
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_GROW) != 0 || write(fd, "!", 1) != -1 ||
        errno != EPERM || jnox_shm_size(fd) != 4 || lseek(alias, 0, SEEK_CUR) != 4) return 113;
    if (lseek(fd, 0, SEEK_SET) != 0 || read(fd, NULL, 2) != -1 || errno != EFAULT ||
        write(fd, NULL, 1) != -1 || errno != EFAULT || lseek(alias, 0, SEEK_CUR) != 0) return 114;
    if (write(other, data, sizeof data) != 256 || lseek(other, 0, SEEK_SET) != 0 ||
        read(other, data, sizeof data) != 256 || read(other, data, 1) != 0) return 115;
    if (lseek(other, 0, SEEK_SET) != 0 || write(other, "abcde", 5) != 5 ||
        ftruncate(other, 2) != 0 || ftruncate(other, 5) != 0 ||
        lseek(other, 0, SEEK_SET) != 0 || read(other, data, 5) != 5 ||
        memcmp(data, "ab\0\0\0", 5) != 0) return 121;
    if (close(fd) != 0 || lseek(fd, 0, SEEK_CUR) != -1 || errno != EBADF ||
        read(alias, data, 4) != 4 || memcmp(data, "xxxy", 4) != 0) return 116;
    if (close(alias) != 0 || close(ro) != 0 || close(other) != 0) return 117;
    int fixed = jnox_shm_create(4);
    if (fixed < 0 || read(fixed, data, 1) != -1 || errno != ENOTSUP ||
        write(fixed, "!", 1) != -1 || errno != ENOTSUP ||
        lseek(fixed, 0, SEEK_SET) != -1 || errno != ENOTSUP || close(fixed) != 0) return 118;
    fd = memfd_create("reuse", flags);
    if (fd < 0 || lseek(fd, 0, SEEK_CUR) != 0 ||
        read(fd, data, sizeof data) != 0 || close(fd) != 0) return 119;
    return 76;
}
