#include <errno.h>
#include <signal.h>
#include <unistd.h>
#include <jnox/ipc.h>
#include <jnox/native_abi.h>
#include <jnox/process.h>

int main(void)
{
    char *live[] = {"live", NULL}, *orphan[] = {"orphan", NULL};
    int pipefd[2], status;
    unsigned char byte;
    errno = EBUSY;
    if (getpid() != 3 || kill(3, 0) != 0 || errno != EBUSY) return 101;
    if (kill(1, 0) != -1 || errno != ESRCH) return 102;
    if (kill(0, 0) != -1 || errno != ENOTSUP) return 103;
    if (kill(-1, 0) != -1 || errno != ENOTSUP) return 104;
    if (kill(3, -1) != -1 || errno != EINVAL) return 105;
    if (kill(3, 32) != -1 || errno != EINVAL) return 106;
    if (jnox_ipc_trap(JNOX_CALL_PROCESS_SIGNAL, 3, 0, 1).status != -EINVAL) return 107;

    // The first child's stdout is the only other writer. EOF establishes exit
    // before probing the retained zombie, independently of dispatch order.
    if (pipe(pipefd) || jnox_spawn(live[0], live, -1, pipefd[1]) != 1) return 108;
    if (close(pipefd[1]) || read(pipefd[0], &byte, 1) != 0 || close(pipefd[0])) return 109;
    if (kill(1, 0)) return 110;
    if (jnox_waitpid(1, &status) != 1 || status != 81) return 111;
    if (kill(1, 0) != -1 || errno != ESRCH) return 112;

    if (pipe(pipefd) || jnox_spawn(orphan[0], orphan, pipefd[0], -1) != 2) return 113;
    // Child 2 is blocked on this empty pipe and observes EOF on our exit.
    return kill(2, 0) == 0 ? 80 : 114;
}
