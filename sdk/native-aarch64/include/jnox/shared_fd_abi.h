#ifndef JNOX_SHARED_FD_ABI_H
#define JNOX_SHARED_FD_ABI_H
/* Existing shared-file duplication call. x1=0 preserves read-only attenuation. */
#define JNOX_CALL_SHM_DUP 188
#define JNOX_SHM_DUP_PLAIN    0x4a464400u
#define JNOX_SHM_DUP_CLOEXEC  0x4a464401u
#define JNOX_SHM_DUP_RESERVED 0x4a464402u
#endif
