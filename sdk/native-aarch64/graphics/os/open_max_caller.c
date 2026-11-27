#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <unistd.h>
#include "jnox/shm.h"

int main(void)
{
    errno = EDOM;
    long limit = sysconf(_SC_OPEN_MAX);
    if (limit != 104 || errno != EDOM) return 101;
    if (sysconf(-1) != -1 || errno != EINVAL) return 102;
    if (jnox_socket_broker(1)) return 103;
    int sockets[4];
    for (int i = 0; i < 4; ++i) {
        sockets[i] = socket(AF_UNIX, SOCK_STREAM, 0);
        if (sockets[i] != 64 + i || sockets[i] >= limit) return 104;
    }
    errno = 0;
    if (socket(AF_UNIX, SOCK_STREAM, 0) != -1 || errno != EMFILE) return 105;
    int backing = jnox_shm_create(4096);
    if (backing != 96) return 106;
    int highest = fcntl(backing, F_DUPFD, (int)limit - 1);
    if (highest != 103 || jnox_shm_size(highest) != 4096) return 107;
    errno = 0;
    if (fcntl(backing, F_DUPFD, (int)limit) != -1 || errno != EINVAL) return 108;
    if (close(highest) || close(backing)) return 109;
    for (int i = 0; i < 4; ++i) {
        if (close(sockets[i])) return 110;
    }
    return 76;
}
