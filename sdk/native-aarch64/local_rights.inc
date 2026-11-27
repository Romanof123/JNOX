/* Included in the opt-in local socket client. User pointers stay in EL0. */
#include <jnox/shm.h>

_Static_assert(sizeof(struct cmsghdr) == 16 && sizeof(struct msghdr) == 56 &&
               sizeof(struct iovec) == 16, "native LP64 message layout");

enum { BACKING_BASE = 96, BACKING_COUNT = 8 };
static int backing_range(int fd) {
    return fd >= BACKING_BASE && fd < BACKING_BASE + BACKING_COUNT;
}
static long backing_call(unsigned op, long first, long second) {
    long r = jnox_ipc_trap(op, first, second, 0).status;
    return r < 0 ? error(-r) : r;
}
int jnox_shm_create(size_t length) {
    return (int)backing_call(180, (long)length, 0);
}
void *jnox_shm_map(int fd, int prot) {
    return (void *)backing_call(181, fd, prot);
}
int jnox_shm_unmap(void *address) {
    return (int)backing_call(182, (long)address, 0);
}
int jnox_shm_close(int fd) {
    return (int)backing_call(183, fd, 0);
}
long jnox_shm_size(int fd) {
    return backing_call(187, fd, 0);
}
int jnox_shm_dup_readonly(int fd) {
    return (int)backing_call(188, fd, 0);
}
static long backing_io(int fd) {
    return jnox_shm_size(fd) < 0 ? -1 : error(ENOTSUP);
}

/* Validate the vector count and all lengths before a receive consumes bytes
 * or a send takes an in-flight reference. A stream call may transfer a prefix. */
static long prepare_message(struct transaction *t, unsigned op, int fd,
                            const struct msghdr *msg, int flags) {
    if (flags & ~MSG_DONTWAIT) return -ENOTSUP;
    if (!msg) return -EFAULT;
    if ((uintptr_t)msg % _Alignof(struct msghdr)) return -EINVAL;
    if (msg->msg_name || msg->msg_namelen) return -ENOTSUP;
    if (msg->msg_iovlen > JNOX_MSG_IOV_MAX) return -EMSGSIZE;
    if ((msg->msg_iovlen && !msg->msg_iov) ||
        (msg->msg_controllen && !msg->msg_control)) return -EFAULT;
    if (msg->msg_iovlen && (uintptr_t)msg->msg_iov % _Alignof(struct iovec)) return -EINVAL;
    if (msg->msg_controllen >= CMSG_LEN(sizeof(int)) &&
        (uintptr_t)msg->msg_control % _Alignof(struct cmsghdr)) return -EINVAL;
    size_t length = 0;
    for (size_t i = 0; i < msg->msg_iovlen; ++i) {
        const struct iovec *v = &msg->msg_iov[i];
        if (v->iov_len && !v->iov_base) return -EFAULT;
        if (v->iov_len > (size_t)__PTRDIFF_MAX__ - length) return -EMSGSIZE;
        length += v->iov_len;
    }
    long r = prepare(t, op, fd);
    if (r < 0) return r;
    if (descriptors[t->slot].family != AF_UNIX || descriptors[t->slot].kind != SOCK_STREAM)
        return -ENOTSUP;
    t->nonblock |= !!(flags & MSG_DONTWAIT);
    return length < DATA_MAX ? (long)length : DATA_MAX;
}

static long sending_right(const struct msghdr *msg) {
    if (!msg->msg_controllen) return 0;
    if (msg->msg_controllen < CMSG_LEN(sizeof(int)) ||
        msg->msg_controllen > CMSG_SPACE(sizeof(int))) return -EINVAL;
    struct cmsghdr h;
    __builtin_memcpy(&h, msg->msg_control, sizeof(h));
    if (h.cmsg_len != CMSG_LEN(sizeof(int))) return -EINVAL;
    if (h.cmsg_level != SOL_SOCKET || h.cmsg_type != SCM_RIGHTS) return -ENOTSUP;
    int fd;
    __builtin_memcpy(&fd, CMSG_DATA(msg->msg_control), sizeof(fd));
    return backing_range(fd) ? fd : -EBADF;
}

ssize_t sendmsg(int fd, const struct msghdr *msg, int flags) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long n = prepare_message(&t, OP_SEND, fd, msg, flags);
    if (n < 0) return finish(n);
    long right = sending_right(msg);
    if (right < 0) return finish(right);
    if (right) {
        if (!n) return finish(-EINVAL);
        t.op = OP_SEND_RIGHTS;
        t.arg = (unsigned)right;
    }
    unsigned char payload[DATA_MAX];
    unsigned used = 0;
    for (size_t i = 0; i < msg->msg_iovlen && used < (unsigned)n; ++i) {
        const unsigned char *src = msg->msg_iov[i].iov_base;
        for (size_t j = 0; j < msg->msg_iov[i].iov_len && used < (unsigned)n; ++j)
            payload[used++] = src[j];
    }
    t.data = payload;
    t.datalen = used;
    long r = commit(&t);
    return finish(r > n || (right && r == 0) ? -EIO : r);
}

/* The authenticated COMMIT reply identifies any installed FD before the
 * optional META and READ calls. Only that freshly installed FD is rolled back. */
static long receive_message(struct msghdr *msg, const struct transaction *t, long result) {
    unsigned count = result & 255;
    int fd = (result >> 8) & 255;
    if (fd && !backing_range(fd)) return -EIO;
    long status = -EIO;
    if ((result >> 16) || count > t->arg || (fd && (!count || !t->accept_right))) goto failed;
    long meta = rpc(0x14ULL << 56);
    if (meta < 0) { status = meta; goto failed; }
    unsigned truncated = (meta >> 32) & 1;
    uint64_t expected = count | ((uint64_t)count << 8) | ((uint64_t)fd << 24) |
                        ((uint64_t)truncated << 32);
    if ((uint64_t)meta != expected || (truncated && (!count || fd))) goto failed;
    unsigned char payload[DATA_MAX];
    status = response_bytes(payload, count);
    if (status < 0) goto failed;

    unsigned done = 0;
    for (size_t i = 0; i < msg->msg_iovlen && done < count; ++i) {
        unsigned char *dst = msg->msg_iov[i].iov_base;
        for (size_t j = 0; j < msg->msg_iov[i].iov_len && done < count; ++j)
            dst[j] = payload[done++];
    }
    if (fd) {
        struct cmsghdr h = {CMSG_LEN(sizeof(int)), SOL_SOCKET, SCM_RIGHTS};
        unsigned char *dst = msg->msg_control;
        __builtin_memcpy(dst, &h, sizeof(h));
        __builtin_memcpy(CMSG_DATA(dst), &fd, sizeof(fd));
        if (msg->msg_controllen > CMSG_SPACE(sizeof(fd))) msg->msg_controllen = CMSG_SPACE(sizeof(fd));
        for (size_t i = h.cmsg_len; i < msg->msg_controllen; ++i) dst[i] = 0;
    } else {
        msg->msg_controllen = 0;
    }
    msg->msg_flags = truncated ? MSG_CTRUNC : 0;
    return (long)count;
failed:
    if (fd) (void)jnox_shm_close(fd);
    return status;
}

ssize_t recvmsg(int fd, struct msghdr *msg, int flags) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long n = prepare_message(&t, OP_RECV_RIGHTS, fd, msg, flags);
    if (n < 0) return finish(n);
    t.arg = (unsigned)n;
    t.accept_right = msg->msg_controllen >= CMSG_LEN(sizeof(int));
    long r = commit(&t);
    return finish(r < 0 ? r : receive_message(msg, &t, r));
}
