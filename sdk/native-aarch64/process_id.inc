/* Shared by the native C SDK and the Picolibc graphics personality. */
#include "jnox/ipc.h"
#include "jnox/native_abi.h"

static pid_t current_process_id(int parent)
{
    jnox_ipc_registers r = jnox_ipc_trap(JNOX_CALL_PROCESS_IDENTITY, 0, 0, 0);
    if (r.status == 0) return (pid_t)(parent ? r.x2 : r.x1);
    if (r.status == -38) errno = ENOSYS;
    else if (r.status == -95) errno = ENOTSUP;
    else errno = EIO;
    return (pid_t)-1;
}

pid_t getpid(void) { return current_process_id(0); }
pid_t getppid(void) { return current_process_id(1); }

static int process_group_call(int op, pid_t pid, pid_t pgid)
{
    jnox_ipc_registers r = jnox_ipc_trap(JNOX_CALL_PROCESS_GROUP, op, (intptr_t)pid, (intptr_t)pgid);
    if (r.status == 0) return (int)r.x1;
    switch (r.status) {
    case -1: errno = EPERM; break;
    case -3: errno = ESRCH; break;
    case -13: errno = EACCES; break;
    case -22: errno = EINVAL; break;
    case -38: errno = ENOSYS; break;
    case -95: errno = ENOTSUP; break;
    default: errno = EIO; break;
    }
    return -1;
}

pid_t getpgrp(void) { return process_group_call(0, 0, 0); }
int setpgid(pid_t pid, pid_t pgid) { return process_group_call(1, pid, pgid); }
