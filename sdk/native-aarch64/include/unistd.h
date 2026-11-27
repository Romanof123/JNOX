#ifndef JNOX_UNISTD_H
#define JNOX_UNISTD_H
#include <stddef.h>
typedef __PTRDIFF_TYPE__ ssize_t;
typedef long off_t;
typedef int pid_t;
pid_t getpid(void);
pid_t getppid(void);
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2
off_t lseek(int fd, off_t offset, int whence);
int ftruncate(int fd, off_t length);
#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2
ssize_t read(int fd, void *buffer, size_t count);
ssize_t write(int fd, const void *buffer, size_t count);
int close(int fd);
int pipe(int fds[2]);
int pipe2(int fds[2], int flags);
int dup(int fd);
int dup2(int fd, int target);
int unlink(const char *path);
#endif
