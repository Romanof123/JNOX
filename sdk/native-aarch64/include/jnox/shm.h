#ifndef JNOX_SHM_H
#define JNOX_SHM_H
#include <stddef.h>

#define JNOX_SHM_READ 1
#define JNOX_SHM_WRITE 2
#define JNOX_SHM_MAX_BYTES 307200
#define JNOX_SHM_MAP_FAILED ((void *)-1)

int jnox_shm_create(size_t length);
int jnox_shm_dup_readonly(int fd);
long jnox_shm_size(int fd);
/* prot is JNOX_SHM_READ or JNOX_SHM_READ | JNOX_SHM_WRITE. */
void *jnox_shm_map(int fd, int prot);
int jnox_shm_unmap(void *address);
int jnox_shm_close(int fd);
#endif
