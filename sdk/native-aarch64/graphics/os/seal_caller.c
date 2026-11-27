#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include <errno.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <unistd.h>
#include "jnox/shm.h"

_Static_assert(F_DUPFD == 0 && F_DUPFD_CLOEXEC == 14, "Picolibc descriptor commands");

int main(void)
{
    int fd = memfd_create("xwayland-shm", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (fd < 0 || jnox_shm_size(fd) != 0 || fcntl(fd, F_GET_SEALS) != 0) return 101;
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK) != 0 ||
        fcntl(fd, F_GET_SEALS) != F_SEAL_SHRINK) return 102;
    if (ftruncate(fd, 4096) != 0 || jnox_shm_size(fd) != 4096) return 103;
    errno = 0;
    if (ftruncate(fd, 1) != -1 || errno != EPERM || jnox_shm_size(fd) != 4096) return 104;
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_SEAL) != 0) return 105;
    errno = 0;
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_GROW) != -1 || errno != EPERM ||
        fcntl(fd, F_GET_SEALS) != (F_SEAL_SEAL | F_SEAL_SHRINK)) return 106;
    if (close(fd) != 0) return 107;
    errno = 0;
    if (fcntl(fd, F_GET_SEALS) != -1 || errno != EBADF) return 108;
    return 75;
}
