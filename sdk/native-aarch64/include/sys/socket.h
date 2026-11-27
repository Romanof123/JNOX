#ifndef JNOX_SYS_SOCKET_H
#define JNOX_SYS_SOCKET_H
#include <unistd.h>
#include <stdint.h>
#include <sys/uio.h>

typedef unsigned int socklen_t;
typedef unsigned short sa_family_t;
struct sockaddr { sa_family_t sa_family; char sa_data[14]; };
struct msghdr {
    void *msg_name;
    socklen_t msg_namelen;
    struct iovec *msg_iov;
    size_t msg_iovlen;
    void *msg_control;
    size_t msg_controllen;
    int msg_flags;
};
struct cmsghdr { size_t cmsg_len; int cmsg_level, cmsg_type; };
#define SOL_SOCKET 1
#define SCM_RIGHTS 1
#define MSG_CTRUNC 0x08
#define JNOX_MSG_IOV_MAX 8
#define CMSG_ALIGN(n) (((size_t)(n) + sizeof(size_t) - 1) & ~(sizeof(size_t) - 1))
#define CMSG_LEN(n) (CMSG_ALIGN(sizeof(struct cmsghdr)) + (size_t)(n))
#define CMSG_SPACE(n) (CMSG_ALIGN(sizeof(struct cmsghdr)) + CMSG_ALIGN(n))
#define CMSG_DATA(c) ((unsigned char *)(c) + CMSG_ALIGN(sizeof(struct cmsghdr)))

static inline struct cmsghdr *CMSG_FIRSTHDR(const struct msghdr *m) {
    if (!m || !m->msg_control || ((uintptr_t)m->msg_control % sizeof(size_t)) ||
        m->msg_controllen < sizeof(struct cmsghdr)) return NULL;
    return (struct cmsghdr *)m->msg_control;
}
static inline struct cmsghdr *CMSG_NXTHDR(const struct msghdr *m, const struct cmsghdr *c) {
    if (!CMSG_FIRSTHDR(m) || !c) return NULL;
    uintptr_t base = (uintptr_t)m->msg_control, at = (uintptr_t)c;
    if (at < base || at % sizeof(size_t) || at - base > m->msg_controllen) return NULL;
    size_t left = m->msg_controllen - (at - base);
    if (left < sizeof(*c) || c->cmsg_len < sizeof(*c) || c->cmsg_len > left ||
        c->cmsg_len > (size_t)-1 - (sizeof(size_t) - 1)) return NULL;
    size_t step = CMSG_ALIGN(c->cmsg_len);
    if (step > left - sizeof(*c) || at > (uintptr_t)-1 - step) return NULL;
    return (struct cmsghdr *)(at + step);
}
#define PF_LOCAL AF_UNIX
#define AF_UNIX 1
#define AF_INET 2
#define AF_LOCAL AF_UNIX
#define SOCK_STREAM 1
#define SOCK_DGRAM 2
#define SOCK_SEQPACKET 5
#define SOCK_CLOEXEC 0x80000
#define SOCK_NONBLOCK 0x800
#define MSG_NOSIGNAL 0x4000
#define MSG_TRUNC 0x20
#define MSG_DONTWAIT 0x40
/* Install SCM_RIGHTS descriptors with receiver-local FD_CLOEXEC. */
#define MSG_CMSG_CLOEXEC 0x40000000
#define SHUT_RD 0
#define SHUT_WR 1
#define SHUT_RDWR 2

int jnox_socket_broker(uint64_t pid);
int socket(int domain, int type, int protocol);
int bind(int fd, const struct sockaddr *addr, socklen_t len);
int connect(int fd, const struct sockaddr *addr, socklen_t len);
int listen(int fd, int backlog);
int accept(int fd, struct sockaddr *addr, socklen_t *len);
int getsockname(int fd, struct sockaddr *addr, socklen_t *len);
int getpeername(int fd, struct sockaddr *addr, socklen_t *len);
ssize_t send(int fd, const void *buf, size_t len, int flags);
ssize_t recv(int fd, void *buf, size_t len, int flags);
ssize_t sendmsg(int fd, const struct msghdr *message, int flags);
ssize_t recvmsg(int fd, struct msghdr *message, int flags);
ssize_t sendto(int fd, const void *buf, size_t len, int flags,
               const struct sockaddr *addr, socklen_t addrlen);
ssize_t recvfrom(int fd, void *buf, size_t len, int flags,
                 struct sockaddr *addr, socklen_t *addrlen);
int shutdown(int fd, int how);
#endif
