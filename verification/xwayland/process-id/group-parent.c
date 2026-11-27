#include <errno.h>
#include <jnox/ipc.h>
#include <jnox/native_abi.h>
#include <jnox/process.h>
#include <unistd.h>

int main(void)
{
    char *live[] = {"live", NULL}, *orphan[] = {"orphan", NULL};
    int first[2], second[2], status;
    if (getpid() != 3 || getpgrp() != 3 || setpgid(0, 0) || getpgrp() != 3) return 131;
    if (setpgid(1, 0) != -1 || errno != ESRCH) return 132;
    if (setpgid(0, 1) != -1 || errno != EPERM || getpgrp() != 3) return 133;
    if (setpgid(0, -1) != -1 || errno != EINVAL) return 134;
    jnox_ipc_registers bad = jnox_ipc_trap(JNOX_CALL_PROCESS_GROUP, 0, 1, 0);
    if (bad.status != -EINVAL || bad.x1 != 0 || bad.x2 != 0) return 135;
    if (pipe(first) || jnox_spawn(live[0], live, first[0], -1) != 1) return 136;
    if (setpgid(1, 3) != -1 || errno != EACCES || setpgid(getpid(), 1)) return 137;
    if (pipe(second) || jnox_spawn(orphan[0], orphan, second[0], -1) != 2) return 138;
    if (setpgid(2, 3) != -1 || errno != EACCES || close(first[1])) return 139;
    if (jnox_waitpid(1, &status) != 1 || status != 91) return 140;
    if (setpgid(1, 1) != -1 || errno != ESRCH) return 141;
    // The leader is reaped. Slot 2 is still a member of group 1.
    if (setpgid(0, 0) || getpgrp() != 3 || setpgid(0, 1) || getpgrp() != 1) return 142;
    // Leave group 1 again. Exit closes the other writer, waking slot 2.
    return setpgid(0, 0) == 0 && getpgrp() == 3 ? 90 : 143;
}
