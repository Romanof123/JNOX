#ifndef JNOX_SYS_IOCTL_H
#define JNOX_SYS_IOCTL_H

#define FIONREAD 0x541BUL

#ifdef __cplusplus
extern "C" {
#endif
/* The local socket profile exposes only the pointer-result FIONREAD request. */
int ioctl(int fd, unsigned long request, void *result);
#ifdef __cplusplus
}
#endif
#endif
