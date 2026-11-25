#ifndef JNOX_ERRNO_H
#define JNOX_ERRNO_H
#define EPERM 1
#define ESRCH 3
#define EAGAIN 11
#define EBUSY 16
#define EDEADLK 35
#define EINTR 4
#define EFAULT 14
#define EOVERFLOW 75
#define EINVAL 22
#define ENOSYS 38
#define ENOTSUP 95
#define ENOENT 2
#define ENXIO 6
#define EBADF 9
#define EACCES 13
#define EEXIST 17
#define ENFILE 23
#define EMFILE 24
#define ENOSPC 28
#define EPIPE 32
#define ENAMETOOLONG 36
#define ETIMEDOUT 110
#define EIO 5
#define EWOULDBLOCK EAGAIN
#define EDESTADDRREQ 89
#define EMSGSIZE 90
#define EPROTOTYPE 91
#define ESOCKTNOSUPPORT 94
#define EOPNOTSUPP ENOTSUP
#define EADDRINUSE 98
#define EISCONN 106
#define ENOTCONN 107
#define ECONNREFUSED 111
int *__jnox_errno_location(void);
#define errno (*__jnox_errno_location())
#endif
