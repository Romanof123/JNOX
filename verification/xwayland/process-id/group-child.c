#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <string.h>
#include <unistd.h>
#include "jnox/ipc.h"

static int waited_for_eof(void)
{
    unsigned char byte;
    return jnox_ipc_trap(46, STDIN_FILENO, (uintptr_t)&byte, 1).status == 0;
}

int main(int argc, char **argv)
{
    if (argc != 1 || getppid() != 3) return 151;
    errno = EDOM;
    if (strcmp(argv[0], "live") == 0) {
        if (getpid() != 1 || getpgrp() != 3 || errno != EDOM) return 152;
        if (setpgid(0, 0) || getpgrp() != 1 || errno != EDOM) return 153;
        return waited_for_eof() && getpgrp() == 1 ? 91 : 154;
    }
    if (strcmp(argv[0], "orphan") != 0 || getpid() != 2 || getpgrp() != 1) return 155;
    if (setpgid(1, 2) != -1 || errno != ESRCH || getpgrp() != 1) return 156;
    if (!waited_for_eof() || getppid() != 0 || getpgrp() != 1) return 157;
    // The previous group 3 and (after our move) group 1 have no members.
    if (setpgid(0, 3) != -1 || errno != EPERM || getpgrp() != 1) return 158;
    if (setpgid(getpid(), 2) || getpgrp() != 2) return 159;
    return setpgid(0, 1) == -1 && errno == EPERM && getpgrp() == 2 ? 92 : 160;
}
