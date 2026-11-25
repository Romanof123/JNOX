#include <errno.h>
#include <jnox/ipc.h>
#include <sys/un.h>

/* Each native process has its own BSS and its own authenticated broker slot.
 * Fail a concurrent call rather than interleave BEGIN/CHUNK/COMMIT/READ. */
enum { FD_BASE = 64, FD_COUNT = 4, PATH_MAX_BYTES = 31, DATA_MAX = 64 };
enum { OP_SOCKET = 1, OP_BIND, OP_CONNECT, OP_LISTEN, OP_ACCEPT, OP_SEND,
       OP_RECV, OP_SHUTDOWN, OP_CLOSE };
static struct descriptor { unsigned char kind, nonblock; } descriptors[FD_COUNT];
static uint64_t broker_pid;
static unsigned char configured, active;

static long error(long code) {
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
    unsigned op, slot, arg, nonblock;
    const unsigned char *path, *data;
    unsigned pathlen, datalen;
};

/* BEGIN and CHUNK succeed with status zero. Only COMMIT can block in service. */
static long commit(const struct transaction *t) {
    uint64_t begin = (0x11ULL << 56) | ((uint64_t)t->op << 48)
        | ((uint64_t)t->slot << 40) | ((uint64_t)t->arg << 32)
        | ((uint64_t)t->pathlen << 24) | ((uint64_t)t->datalen << 16)
        | ((uint64_t)t->nonblock << 8);
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
    if (op == OP_SOCKET) return 0;
    if (fd < FD_BASE || fd >= FD_BASE + FD_COUNT) return -EBADF;
    t->slot = (unsigned)(fd - FD_BASE);
    if (!descriptors[t->slot].kind) return -EBADF;
    t->nonblock = descriptors[t->slot].nonblock;
    return 0;
}

static long new_fd(long result, unsigned kind, unsigned nonblock) {
    if (result < 0) return result;
    if (result >= FD_COUNT || descriptors[result].kind) return -EIO;
    descriptors[result] = (struct descriptor){kind, nonblock};
    return FD_BASE + result;
}
int socket(int domain, int type, int protocol) {
    unsigned kind = type & ~SOCK_NONBLOCK;
    if (domain != AF_UNIX || protocol != 0 ||
        (kind != SOCK_STREAM && kind != SOCK_DGRAM && kind != SOCK_SEQPACKET))
        return error(ENOTSUP);
    if (!enter()) return -1;
    struct transaction t = {0};
    long r = prepare(&t, OP_SOCKET, 0);
    if (r < 0) return finish(r);
    t.arg = kind;
    t.nonblock = !!(type & SOCK_NONBLOCK);
    return finish(new_fd(commit(&t), kind, t.nonblock));
}

/* Use byte copies of the common family field so sockaddr casts need no
 * aliasing assumption. The supplied length bounds every pathname read. */
static long address(struct transaction *t, const struct sockaddr *addr, socklen_t len) {
    const unsigned base = __builtin_offsetof(struct sockaddr_un, sun_path);
    if (!addr || len <= base || len > sizeof(struct sockaddr_un)) return -EINVAL;
    sa_family_t family;
    __builtin_memcpy(&family, addr, sizeof family);
    if (family != AF_UNIX) return -ENOTSUP;
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
        r = new_fd(r, descriptors[t.slot].kind, 0);
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
    if (flags & ~MSG_DONTWAIT) return error(ENOTSUP);
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

static long read_response(unsigned char *data, unsigned cap, long count, int flags,
                          struct sockaddr *addr, socklen_t *addrlen) {
    long meta = rpc(0x14ULL << 56);
    if (meta < 0) return meta;
    unsigned used = meta & 255, full = (meta >> 8) & 255, path = (meta >> 16) & 255;
    if ((meta >> 24) || used != (unsigned long)count || used > cap ||
        full < used || full > DATA_MAX || path > PATH_MAX_BYTES) return -EIO;
    struct sockaddr_un source = {.sun_family = AF_UNIX};
    unsigned total = used + path;
    unsigned char payload[DATA_MAX];
    for (unsigned off = 0; off < total; off += 6) {
        long word = rpc((0x15ULL << 56) | ((uint64_t)off << 48));
        if (word < 0) return word;
        if ((unsigned long)word >> 48) return -EIO;
        for (unsigned j = 0; j < 6 && off + j < total; ++j) {
            unsigned i = off + j;
            unsigned char byte = (unsigned long)word >> (8 * j);
            if (i < used) payload[i] = byte;
            else source.sun_path[i - used] = (char)byte;
        }
    }
    for (unsigned i = 0; i < used; ++i) data[i] = payload[i];
    if (addr) {
        socklen_t actual = __builtin_offsetof(struct sockaddr_un, sun_path) + path + !!path;
        socklen_t copy = *addrlen < actual ? *addrlen : actual;
        for (unsigned i = 0; i < copy; ++i)
            ((unsigned char *)addr)[i] = ((const unsigned char *)&source)[i];
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
    if (r >= 0) r = read_response(buf, t.arg, r, flags, addr, addrlen);
    return finish(r);
}
ssize_t recv(int fd, void *buf, size_t len, int flags) {
    return recvfrom(fd, buf, len, flags, 0, 0);
}

extern int __real_close(int);
extern ssize_t __real_read(int, void *, size_t);
extern ssize_t __real_write(int, const void *, size_t);
static int socket_range(int fd) { return fd >= FD_BASE && fd < FD_BASE + FD_COUNT; }
int __wrap_close(int fd) {
    return socket_range(fd) ? simple(OP_CLOSE, fd, 0) : __real_close(fd);
}
ssize_t __wrap_read(int fd, void *buf, size_t n) {
    return socket_range(fd) ? recv(fd, buf, n, 0) : __real_read(fd, buf, n);
}
ssize_t __wrap_write(int fd, const void *buf, size_t n) {
    return socket_range(fd) ? send(fd, buf, n, 0) : __real_write(fd, buf, n);
}
