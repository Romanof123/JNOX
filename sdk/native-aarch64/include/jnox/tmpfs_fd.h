#ifndef JNOX_TMPFS_FD_H
#define JNOX_TMPFS_FD_H
#include <stddef.h>
static inline int jnox_tmp_fd(int fd) { return fd >= 32 && fd < 40; }
long jnox_tmp_read(int fd, void *buffer, size_t size);
long jnox_tmp_write(int fd, const void *buffer, size_t size);
long jnox_tmp_seek(int fd, long offset, int whence);
int jnox_tmp_close(int fd);
int jnox_tmp_fcntl(int fd, int command, int argument);
int jnox_tmp_dup2(int fd, int target);
#endif
