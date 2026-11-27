#include <errno.h>
#include <jnox/ipc.h>
#include <sys/un.h>
#include <netinet/in.h>

_Static_assert(sizeof(struct sockaddr_in) == 16 &&
               __builtin_offsetof(struct sockaddr_in, sin_port) == 2 &&
               __builtin_offsetof(struct sockaddr_in, sin_addr) == 4,
               "native IPv4 address layout");

/* Each native process has its own BSS and its own authenticated broker slot.
 * Fail a concurrent call rather than interleave BEGIN/CHUNK/COMMIT/READ. */
enum { FD_BASE = 64, FD_COUNT = 4, PATH_MAX_BYTES = 31, DATA_MAX = 64 };
enum { OP_SOCKET = 1, OP_BIND, OP_CONNECT, OP_LISTEN, OP_ACCEPT, OP_SEND,
       OP_RECV, OP_SHUTDOWN, OP_CLOSE, OP_SOCKET4, OP_BIND4, OP_CONNECT4,
       OP_SEND4, OP_RECV4, OP_SEND_RIGHTS, OP_RECV_RIGHTS };
static struct descriptor { unsigned char kind, nonblock, family, cloexec; } descriptors[FD_COUNT];
static uint64_t broker_pid;
static unsigned char configured, active;

static long error(long code) {
#ifdef JNOX_GRAPHICS_OS
    extern void jnox_graphics_errno(int);
    jnox_graphics_errno((int)code);
#endif
    errno = (int)code;
    return -1;
}
static int enter(void) {
    if (__atomic_test_and_set(&active, __ATOMIC_ACQUIRE)) {
        error(EBUSY);
        return 0;
    }
    return 1;
}
static long finish(long value) {
    __atomic_clear(&active, __ATOMIC_RELEASE);
    return value < 0 ? error(value >= -4095 ? -value : EIO) : value;
}
int jnox_socket_broker(uint64_t pid) {
    if (!enter()) return -1;
    if (configured && broker_pid != pid) return finish(-EBUSY);
    broker_pid = pid;
    configured = 1;
    return finish(0);
}
static long rpc(uint64_t request) {
    jnox_ipc_response r = jnox_ipc_call(broker_pid, request, UINT64_MAX);
    return r.status ? r.status : (long)r.word;
}

struct transaction {
    unsigned op, slot, arg, nonblock, accept_right, cloexec;
    const unsigned char *path, *data;
    unsigned pathlen, datalen;
};

/* BEGIN and CHUNK succeed with status zero. Only COMMIT can block in service. */
static long commit(const struct transaction *t) {
    uint64_t begin = (0x11ULL << 56) | ((uint64_t)t->op << 48)
        | ((uint64_t)t->slot << 40) | ((uint64_t)t->arg << 32)
        | ((uint64_t)t->pathlen << 24) | ((uint64_t)t->datalen << 16)
        | ((uint64_t)t->nonblock << 8) | ((uint64_t)t->accept_right << 9) | ((uint64_t)t->cloexec << 10);
    long r = rpc(begin);
    if (r != 0) return r < 0 ? r : -EIO;
    unsigned total = t->pathlen + t->datalen;
    for (unsigned off = 0; off < total; off += 6) {
        uint64_t chunk = (0x12ULL << 56) | ((uint64_t)off << 48);
        for (unsigned j = 0; j < 6 && off + j < total; ++j) {
            unsigned i = off + j;
            unsigned byte = i < t->pathlen ? t->path[i] : t->data[i - t->pathlen];
            chunk |= (uint64_t)byte << (8 * j);
        }
        r = rpc(chunk);
        if (r != 0) {
            (void)rpc(0x16ULL << 56);
            return r < 0 ? r : -EIO;
        }
    }
    return rpc(0x13ULL << 56);
}

static long prepare(struct transaction *t, unsigned op, int fd) {
    if (!configured) return -ENOTCONN;
    t->op = op;
    if (op == OP_SOCKET || op == OP_SOCKET4) return 0;
    if (fd < FD_BASE || fd >= FD_BASE + FD_COUNT) return -EBADF;
    t->slot = (unsigned)(fd - FD_BASE);
    if (!descriptors[t->slot].kind) return -EBADF;
    if (descriptors[t->slot].family == AF_INET) {
        switch (op) {
        case OP_BIND: t->op = OP_BIND4; break;
        case OP_CONNECT: t->op = OP_CONNECT4; break;
        case OP_SEND: t->op = OP_SEND4; break;
        case OP_RECV: t->op = OP_RECV4; break;
        }
    }
    t->nonblock = descriptors[t->slot].nonblock;
    return 0;
}

static long new_fd(long result, unsigned kind, unsigned nonblock, unsigned family, unsigned cloexec) {
    if (result < 0) return result;
    if (result >= FD_COUNT || descriptors[result].kind) return -EIO;
    descriptors[result] = (struct descriptor){kind, nonblock, family, cloexec};
    return FD_BASE + result;
}
int socket(int domain, int type, int protocol) {
    unsigned kind = type & ~(SOCK_NONBLOCK | SOCK_CLOEXEC);
    if (domain == AF_INET) {
        if ((kind != SOCK_STREAM && kind != SOCK_DGRAM) ||
            (protocol && protocol != (kind == SOCK_STREAM ? IPPROTO_TCP : IPPROTO_UDP)))
            return error(ENOTSUP);
    } else if (domain != AF_UNIX || protocol != 0 ||
               (kind != SOCK_STREAM && kind != SOCK_DGRAM && kind != SOCK_SEQPACKET)) {
        return error(ENOTSUP);
    }
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, domain == AF_INET ? OP_SOCKET4 : OP_SOCKET, 0);
    if (r < 0) return finish(r);
    t.arg = kind;
    t.nonblock = !!(type & SOCK_NONBLOCK);
    return finish(new_fd(commit(&t), kind, t.nonblock, domain, !!(type & SOCK_CLOEXEC)));
}

/* Use byte copies of the common family field so sockaddr casts need no
 * aliasing assumption. The supplied length bounds every pathname read. */
static long address(struct transaction *t, const struct sockaddr *addr, socklen_t len) {
    const unsigned base = __builtin_offsetof(struct sockaddr_un, sun_path);
    if (!addr || len <= base || len > sizeof(struct sockaddr_un)) return -EINVAL;
    sa_family_t family;
    __builtin_memcpy(&family, addr, sizeof family);
    if (family != descriptors[t->slot].family) return -ENOTSUP;
    if (family == AF_INET) {
        if (len != sizeof(struct sockaddr_in)) return -EINVAL;
        t->path = (const unsigned char *)addr + __builtin_offsetof(struct sockaddr_in, sin_port);
        t->pathlen = 6;
        return 0;
    }
    t->path = (const unsigned char *)addr + base;
    while (t->pathlen < len - base && t->path[t->pathlen]) ++t->pathlen;
    return t->pathlen && t->pathlen <= PATH_MAX_BYTES ? 0 : -EINVAL;
}
static int named(unsigned op, int fd, const struct sockaddr *addr, socklen_t len) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, op, fd);
    if (r == 0) r = address(&t, addr, len);
    return finish(r < 0 ? r : commit(&t));
}
int bind(int fd, const struct sockaddr *addr, socklen_t len) {
    return named(OP_BIND, fd, addr, len);
}
int connect(int fd, const struct sockaddr *addr, socklen_t len) {
    return named(OP_CONNECT, fd, addr, len);
}
static int simple(unsigned op, int fd, unsigned arg) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, op, fd);
    if (r < 0) return finish(r);
    t.arg = arg;
    r = commit(&t);
    if (op == OP_ACCEPT)
        r = new_fd(r, descriptors[t.slot].kind, 0, descriptors[t.slot].family, 0);
    if (op == OP_CLOSE && r == 0)
        descriptors[t.slot] = (struct descriptor){0};
    return finish(r);
}
int listen(int fd, int backlog) {
    if (backlog < 0 || backlog > 2) return error(EINVAL);
    return simple(OP_LISTEN, fd, (unsigned)backlog);
}
int accept(int fd, struct sockaddr *addr, socklen_t *len) {
    if (addr) return error(ENOTSUP); /* v1 accept has no peer address result */
    (void)len;
    return simple(OP_ACCEPT, fd, 0);
}
int shutdown(int fd, int how) {
    if (how < SHUT_RD || how > SHUT_RDWR) return error(EINVAL);
    return simple(OP_SHUTDOWN, fd, (unsigned)how);
}

ssize_t sendto(int fd, const void *buf, size_t len, int flags,
               const struct sockaddr *addr, socklen_t addrlen) {
    if (flags & ~(MSG_DONTWAIT | MSG_NOSIGNAL)) return error(ENOTSUP);
    if (len && !buf) return error(EFAULT);
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, OP_SEND, fd);
    if (r == 0 && (addr || addrlen)) r = address(&t, addr, addrlen);
    if (r < 0) return finish(r);
    if (descriptors[t.slot].kind != SOCK_STREAM && len > 32) return finish(-EMSGSIZE);
    t.data = buf;
    t.datalen = len > DATA_MAX ? DATA_MAX : (unsigned)len;
    t.nonblock |= !!(flags & MSG_DONTWAIT);
    r = commit(&t);
    return finish(r > (long)t.datalen ? -EIO : r);
}
ssize_t send(int fd, const void *buf, size_t len, int flags) {
    return sendto(fd, buf, len, flags, 0, 0);
}

static long response_bytes(unsigned char *bytes, unsigned total) {
    for (unsigned off = 0; off < total; off += 6) {
        long word = rpc((0x15ULL << 56) | ((uint64_t)off << 48));
        if (word < 0) return word;
        if ((unsigned long)word >> 48) return -EIO;
        for (unsigned j = 0; j < 6 && off + j < total; ++j)
            bytes[off + j] = (unsigned long)word >> (8 * j);
    }
    return 0;
}

static long read_response(struct descriptor desc, unsigned char *data, unsigned cap, long count, int flags,
                          struct sockaddr *addr, socklen_t *addrlen) {
    long meta = rpc(0x14ULL << 56);
    if (meta < 0) return meta;
    unsigned used = meta & 255, full = (meta >> 8) & 255, path = (meta >> 16) & 255;
    if ((meta >> 24) || used != (unsigned long)count || used > cap ||
        full < used || full > DATA_MAX || path > PATH_MAX_BYTES) return -EIO;
    unsigned family = desc.family;
    if (family == AF_INET) {
        if (desc.kind == SOCK_DGRAM && (path != 6 || full > 32)) return -EIO;
        if (desc.kind == SOCK_STREAM && (path || full != used)) return -EIO;
    }
    unsigned char source[sizeof(struct sockaddr_un)] = {0};
    sa_family_t source_family = family;
    __builtin_memcpy(source, &source_family, sizeof source_family);
    unsigned offset = family == AF_INET ? __builtin_offsetof(struct sockaddr_in, sin_port)
                                        : __builtin_offsetof(struct sockaddr_un, sun_path);
    unsigned char payload[DATA_MAX + PATH_MAX_BYTES];
    long result = response_bytes(payload, used + path);
    if (result < 0) return result;
    for (unsigned i = 0; i < path; ++i) source[offset + i] = payload[used + i];
    for (unsigned i = 0; i < used; ++i) data[i] = payload[i];
    if (addr) {
        socklen_t actual = offset + path + !!path;
        if (family == AF_INET) actual = path ? sizeof(struct sockaddr_in) : 0;
        socklen_t copy = *addrlen < actual ? *addrlen : actual;
        for (unsigned i = 0; i < copy; ++i)
            ((unsigned char *)addr)[i] = source[i];
        *addrlen = actual;
    }
    return flags & MSG_TRUNC ? full : used;
}
ssize_t recvfrom(int fd, void *buf, size_t len, int flags,
                 struct sockaddr *addr, socklen_t *addrlen) {
    if (flags & ~(MSG_TRUNC | MSG_DONTWAIT)) return error(ENOTSUP);
    if ((len && !buf) || (addr && !addrlen)) return error(EFAULT);
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, OP_RECV, fd);
    if (r < 0) return finish(r);
    t.arg = len > DATA_MAX ? DATA_MAX : (unsigned)len;
    t.nonblock |= !!(flags & MSG_DONTWAIT);
    r = commit(&t);
    if (r >= 0) r = read_response(descriptors[t.slot], buf, t.arg, r, flags, addr, addrlen);
    return finish(r);
}
ssize_t recv(int fd, void *buf, size_t len, int flags) {
    return recvfrom(fd, buf, len, flags, 0, 0);
}

#include "local_rights.inc"

extern int __real_close(int);
extern ssize_t __real_read(int, void *, size_t);
extern ssize_t __real_write(int, const void *, size_t);
static int socket_range(int fd) { return fd >= FD_BASE && fd < FD_BASE + FD_COUNT; }
int __wrap_close(int fd) {
    if (backing_range(fd)) return jnox_shm_close(fd);
    return socket_range(fd) ? simple(OP_CLOSE, fd, 0) : __real_close(fd);
}
ssize_t __wrap_read(int fd, void *buf, size_t n) {
    if (backing_range(fd)) return backing_io(fd);
    return socket_range(fd) ? recv(fd, buf, n, 0) : __real_read(fd, buf, n);
}
ssize_t __wrap_write(int fd, const void *buf, size_t n) {
    if (backing_range(fd)) return backing_io(fd);
    return socket_range(fd) ? send(fd, buf, n, 0) : __real_write(fd, buf, n);
}

#include "socket_poll.inc"
