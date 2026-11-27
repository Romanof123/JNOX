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
