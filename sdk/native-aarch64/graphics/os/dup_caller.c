/* Compile against Picolibc's public fcntl.h and link the graphics OS archive. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include "jnox/shm.h"

_Static_assert(F_DUPFD == 0 && F_DUPFD_CLOEXEC == 14, "Picolibc 1.8.10 command ABI");

int main(void)
{
    int original = jnox_shm_create(4096);
    if (original != 96) return 101;
#ifdef JNOX_FRAME_DUP
    int atomic = JNOX_FRAME_DUP(original);
#else
    int atomic = fcntl(original, F_DUPFD_CLOEXEC, 0);
#endif
    int ordinary = fcntl(original, F_DUPFD, 101);
    if (atomic != 97 || ordinary != 101) return 102;
    if (fcntl(atomic, F_GETFD) != FD_CLOEXEC || fcntl(ordinary, F_GETFD) != 0) return 103;
    if (fcntl(original, F_DUPFD, -1) != -1 || errno != EINVAL) return 104;
    if (close(original) || jnox_shm_size(atomic) != 4096) return 105;
    if (close(atomic) || close(ordinary)) return 106;
    return 74;
}
