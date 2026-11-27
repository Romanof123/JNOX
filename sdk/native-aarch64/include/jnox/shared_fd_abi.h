#ifndef JNOX_SHARED_FD_ABI_H
#define JNOX_SHARED_FD_ABI_H
/* Highest descriptor class in the native graphics profile. */
#define JNOX_SHM_FD_BASE 96
#define JNOX_SHM_FD_COUNT 8
/* Existing shared-file duplication call. x1=0 preserves read-only attenuation. */
#define JNOX_CALL_SHM_DUP 188
#define JNOX_SHM_DUP_PLAIN    0x4a464400u
#define JNOX_SHM_DUP_CLOEXEC  0x4a464401u
#define JNOX_SHM_DUP_RESERVED 0x4a464402u
/* x0 = fd, x1 = seek selector or (count << 8) | I/O selector, x2 = offset or pointer. */
#define JNOX_CALL_SHM_FILE_IO 202
#define JNOX_SHM_FILE_READ 3u
#define JNOX_SHM_FILE_WRITE 4u
#define JNOX_SHM_FILE_TRANSFER 256u
/* Exact shared slot replacement: x0 = source fd, x1 = destination fd, x2 = 0. */
#define JNOX_CALL_SHM_DUP2 208
#endif
