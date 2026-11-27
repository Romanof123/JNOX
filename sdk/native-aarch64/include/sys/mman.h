#ifndef JNOX_SYS_MMAN_H
#define JNOX_SYS_MMAN_H
#include <unistd.h>
#define MFD_CLOEXEC 1u
#define MFD_ALLOW_SEALING 2u
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_SHARED 1
#define MAP_FAILED ((void *)-1)
int memfd_create(const char *name, unsigned flags);
void *mmap(void *address, size_t length, int prot, int flags, int fd, off_t offset);
int munmap(void *address, size_t length);
#endif
