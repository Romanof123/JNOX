#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h>
#include <string.h>
#include <unistd.h>
#include "jnox/ipc.h"

int main(int argc, char **argv)
{
    if (argc != 1) return 121;
    pid_t own = getpid(), parent = getppid();
    if (parent != 3 || kill(own, 0)) return 122;
    if (kill(parent, 0) != -1 || errno != EPERM) return 123;
    if (kill(own, SIGUSR1) != -1 || errno != ENOTSUP) return 124;
    if (kill(100, 0) != -1 || errno != ESRCH) return 125;
    if (strcmp(argv[0], "live") == 0)
        return own == 1 && kill(2, 0) == -1 && errno == ESRCH ? 81 : 126;
    if (strcmp(argv[0], "orphan") || own != 2) return 127;
    unsigned char byte;
    if (jnox_ipc_trap(46, STDIN_FILENO, (uintptr_t)&byte, 1).status != 0) return 128;
    if (getppid() != 0 || kill(parent, 0) != -1 || errno != ESRCH) return 129;
    return kill(own, 0) == 0 ? 82 : 130;
}
