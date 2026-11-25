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
int fcntl(int fd, int cmd, ...);
int open(const char *path, int flags, ...);
#endif
