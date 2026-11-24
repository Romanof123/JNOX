#ifndef JNOX_UNISTD_H
#define JNOX_UNISTD_H
#include <stddef.h>
typedef __PTRDIFF_TYPE__ ssize_t;
#define STDOUT_FILENO 1
#define STDERR_FILENO 2
ssize_t read(int fd, void *buffer, size_t count);
ssize_t write(int fd, const void *buffer, size_t count);
int close(int fd);
#endif
