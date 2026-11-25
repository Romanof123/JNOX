#ifndef JNOX_UNISTD_H
#define JNOX_UNISTD_H
#include <stddef.h>
typedef __PTRDIFF_TYPE__ ssize_t;
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
