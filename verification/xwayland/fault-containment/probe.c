#include <errno.h>
#include <jnox/shm.h>
#include <sys/socket.h>
#include <unistd.h>

int main(void)
{
    pid_t pid = getpid();
    if (pid == 5 || pid == 4) return 0; /* independent boot peers */
    if (pid != 1 && pid != 2) return 101;
    if (jnox_socket_broker(3) != 0) return 102;
    int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (socket_fd < 0) return 103;

    int fds[4];
    volatile unsigned char *maps[4];
    for (unsigned i = 0; i < 4; ++i) {
        fds[i] = jnox_shm_create(4096);
        if (fds[i] < 0) return 104;
        maps[i] = jnox_shm_map(fds[i], JNOX_SHM_READ | JNOX_SHM_WRITE);
        if (maps[i] == JNOX_SHM_MAP_FAILED || maps[i][0] != 0) return 105;
        maps[i][0] = (unsigned char)(i + 1);
    }
    if (jnox_shm_create(4096) != -1 || errno != ENOSPC) return 106;
    if (pid == 1) {
        /* Intentional software breakpoint inside this diagnostic application. */
        __asm__ volatile("brk #0" ::: "memory");
        return 107;
    }
    for (unsigned i = 0; i < 4; ++i) {
        if (maps[i][0] != i + 1 || jnox_shm_unmap((void *)maps[i]) || jnox_shm_close(fds[i]))
            return 108;
    }
    return close(socket_fd) == 0 ? 72 : 109;
}
