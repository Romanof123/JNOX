#ifndef JNOX_ERRNO_H
#define JNOX_ERRNO_H
#define ESRCH 3
#define EAGAIN 11
#define EDEADLK 35
#define EINTR 4
#define EFAULT 14
#define EOVERFLOW 75
#define EINVAL 22
#define ENOSYS 38
#define ENOTSUP 95
int *__jnox_errno_location(void);
#define errno (*__jnox_errno_location())
#endif
