#ifndef JNOX_SYS_SOCKET_H
#define JNOX_SYS_SOCKET_H
#include <unistd.h>
#include <stdint.h>

typedef unsigned int socklen_t;
typedef unsigned short sa_family_t;
struct sockaddr { sa_family_t sa_family; char sa_data[14]; };
#define AF_UNIX 1
#define AF_LOCAL AF_UNIX
#define SOCK_STREAM 1
#define SOCK_DGRAM 2
#define SOCK_SEQPACKET 5
#define SOCK_NONBLOCK 0x800
#define MSG_TRUNC 0x20
#define MSG_DONTWAIT 0x40
#define SHUT_RD 0
#define SHUT_WR 1
#define SHUT_RDWR 2

int jnox_socket_broker(uint64_t pid);
int socket(int domain, int type, int protocol);
int bind(int fd, const struct sockaddr *addr, socklen_t len);
int connect(int fd, const struct sockaddr *addr, socklen_t len);
int listen(int fd, int backlog);
int accept(int fd, struct sockaddr *addr, socklen_t *len);
ssize_t send(int fd, const void *buf, size_t len, int flags);
ssize_t recv(int fd, void *buf, size_t len, int flags);
ssize_t sendto(int fd, const void *buf, size_t len, int flags,
               const struct sockaddr *addr, socklen_t addrlen);
ssize_t recvfrom(int fd, void *buf, size_t len, int flags,
                 struct sockaddr *addr, socklen_t *addrlen);
int shutdown(int fd, int how);
#endif
