#ifndef JNOX_FCNTL_H
#define JNOX_FCNTL_H
#define O_RDONLY 0
#define O_WRONLY 1
#define O_NONBLOCK 0x800
#define O_CLOEXEC 0x80000
#define FD_CLOEXEC 1
#define F_GETFD 1
#define F_SETFD 2
#define F_GETFL 3
#define F_SETFL 4
#define F_ADD_SEALS 1033
#define F_GET_SEALS 1034
#define F_SEAL_SEAL 1
#define F_SEAL_SHRINK 2
#define F_SEAL_GROW 4
#define F_SEAL_WRITE 8
int fcntl(int fd, int cmd, ...);
int open(const char *path, int flags, ...);
#endif
