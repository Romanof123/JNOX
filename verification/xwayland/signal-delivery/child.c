#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include "jnox/ipc.h"

static volatile sig_atomic_t received;

static void remember(int signal)
{
    if (signal == SIGUSR1 && received == 0) received = 1;
    else received = -1;
}

static long send_byte(unsigned char byte)
{
    return jnox_ipc_trap(47, STDOUT_FILENO, (uintptr_t)&byte, 1).status;
}

int main(int argc, char **argv)
{
    if (argc != 1 || getppid() != 3) return 111;
    if (strcmp(argv[0], "orphan") == 0) {
        unsigned char byte;
        if (getpid() != 2 || jnox_ipc_trap(46, STDIN_FILENO, (uintptr_t)&byte, 1).status) return 112;
        return getppid() == 0 ? 85 : 113;
    }
    if (strcmp(argv[0], "live") || getpid() != 1) return 114;
    struct sigaction action = {0};
    action.sa_handler = remember;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGUSR1, &action, NULL)) return 115;
    // Both targets exist. The sibling cannot send to either its parent or its peer.
    if (kill(2, SIGUSR1) != -1 || errno != EPERM) return 116;
    if (kill(3, SIGUSR1) != -1 || errno != EPERM) return 117;
    struct timespec start, end;
    if (clock_gettime(CLOCK_MONOTONIC, &start) || send_byte('R') != 1) return 118;
    errno = 0;
    unsigned int left = sleep(15);
    if (received != 1 || errno != EINTR || left < 1 || left > 15) return 119;
    if (clock_gettime(CLOCK_MONOTONIC, &end) || end.tv_sec < start.tv_sec) return 120;
    time_t elapsed = end.tv_sec - start.tv_sec;
    if ((elapsed == 0 && end.tv_nsec < start.tv_nsec) ||
        elapsed > 15 || (elapsed == 15 && end.tv_nsec >= start.tv_nsec)) return 121;
    return send_byte((unsigned char)left) == 1 ? 84 : 122;
}
