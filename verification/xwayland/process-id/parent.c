#include <errno.h>
#include <jnox/ipc.h>
#include <jnox/native_abi.h>
#include <jnox/process.h>
#include <unistd.h>

int main(void)
{
    char *live[] = {"live", NULL};
    char *orphan[] = {"orphan", NULL};
    int fds[2], status;
    if (getpid() != 3 || getppid() != 0) return 101;
    jnox_ipc_registers bad = jnox_ipc_trap(JNOX_CALL_PROCESS_IDENTITY, 1, 1, 1);
    if (bad.status != -EINVAL || bad.x1 != 0 || bad.x2 != 0) return 102;
    int child = jnox_spawn(live[0], live, -1, -1);
    if (child != 1 || jnox_waitpid(child, &status) != child || status != 81) return 103;
    if (jnox_spawn(live[0], live, -1, -1) != -1 || errno != EBUSY) return 104;
    if (pipe(fds) || jnox_spawn(orphan[0], orphan, fds[0], -1) != 2) return 105;
    // The orphan child is blocked on the empty pipe. Parent exit closes its last writer.
    return getpid() == 3 && getppid() == 0 ? 80 : 106;
}
