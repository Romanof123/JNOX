#ifndef JNOX_SYS_UIO_H
#define JNOX_SYS_UIO_H
#include <stddef.h>
#include <unistd.h>

struct iovec {
    void *iov_base;
    size_t iov_len;
};

#define JNOX_UIO_MAXIOV 16

#ifdef __cplusplus
extern "C" {
#endif
ssize_t readv(int fd, const struct iovec *iov, int count);
ssize_t writev(int fd, const struct iovec *iov, int count);
#ifdef __cplusplus
}
#endif
#endif
