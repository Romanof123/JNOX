#ifndef JNOX_POLL_H
#define JNOX_POLL_H
typedef unsigned long nfds_t;
struct pollfd { int fd; short events, revents; };
#define POLLIN   0x01
#define POLLOUT  0x04
#define POLLERR  0x08
#define POLLHUP  0x10
#define POLLNVAL 0x20
int poll(struct pollfd *fds, nfds_t count, int timeout_ms);
#endif
