#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <string.h>
#include <unistd.h>
#include "jnox/ipc.h"

int main(int argc, char **argv)
{
    if (argc != 1) return 111;
    errno = EDOM;
    pid_t own = getpid();
    if (getppid() != 3 || errno != EDOM) return 112;
    if (strcmp(argv[0], "live") == 0) return own == 1 && getpid() == own ? 81 : 113;
    if (strcmp(argv[0], "orphan") != 0 || own != 2) return 114;
    // Synchronize through the native pipe ABI. Both identity queries use public Picolibc.
    unsigned char byte;
    if (jnox_ipc_trap(46, STDIN_FILENO, (uintptr_t)&byte, 1).status != 0) return 115;
    return getpid() == own && getppid() == 0 ? 82 : 116;
}
