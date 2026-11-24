#ifndef JNOX_ERRNO_H
#define JNOX_ERRNO_H
#define EINTR 4
#define EINVAL 22
#define ENOSYS 38
int *__jnox_errno_location(void);
#define errno (*__jnox_errno_location())
#endif
