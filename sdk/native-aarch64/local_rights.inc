/* Included in the opt-in local socket client. User pointers stay in EL0. */
#include <jnox/shm.h>
#include <jnox/shared_fd_abi.h>
#include <fcntl.h>
#include <sys/mman.h>
#ifdef JNOX_GRAPHICS_OS
#include <jnox/tmpfs_fd.h>
#endif

_Static_assert(sizeof(struct cmsghdr) == 16 && sizeof(struct msghdr) == 56 &&
               sizeof(struct iovec) == 16, "native LP64 message layout");

static int backing_range(int fd) {
    return fd >= JNOX_SHM_FD_BASE && fd < JNOX_SHM_FD_BASE + JNOX_SHM_FD_COUNT;
}
static long backing_call(unsigned op, long first, long second) {
    long r = jnox_ipc_trap(op, first, second, 0).status;
    return r < 0 ? error(-r) : r;
}
int jnox_shm_create(size_t length) {
    return (int)backing_call(180, (long)length, 0);
}
int jnox_shm_create_resizable(void) {
    return (int)backing_call(189, 0, 0);
}
int jnox_shm_resize(int fd, size_t length) {
    return (int)backing_call(190, fd, (long)length);
}
int jnox_shm_add_seals(int fd, unsigned seals) {
    return (int)backing_call(191, fd, seals);
}
int jnox_shm_get_seals(int fd) {
    return (int)backing_call(192, fd, 0);
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
    return (int)backing_call(JNOX_CALL_SHM_DUP, fd, 0);
}
/* The native image has no exec transition. Anonymous files live in the
 * process FD table and are always retired at process teardown. */
int memfd_create(const char *name, unsigned flags) {
    if (!name) return error(EFAULT);
    if (flags != (MFD_CLOEXEC | MFD_ALLOW_SEALING)) return error(ENOTSUP);
    for (unsigned i = 0; i <= 249; ++i) {
        if (!name[i]) return jnox_shm_create_resizable();
    }
    return error(EINVAL);
}
int ftruncate(int fd, off_t length) {
    if (!backing_range(fd)) return error(ENOTSUP);
    if (length < 0) return error(EINVAL);
    return jnox_shm_resize(fd, (size_t)length);
}
void *mmap(void *address, size_t length, int prot, int flags, int fd, off_t offset) {
    if (!length || length > JNOX_SHM_MAX_BYTES) return (void *)error(EINVAL);
    if (address || offset || flags != MAP_SHARED || (prot != PROT_READ && prot != (PROT_READ | PROT_WRITE)))
        return (void *)error(ENOTSUP);
    return (void *)backing_call(193, fd, (long)(length << 2) | prot);
}
int munmap(void *address, size_t length) {
    if (!length) return error(EINVAL);
    return (int)backing_call(194, (long)address, (long)length);
}

extern int __real_fcntl(int, int, ...);
int __wrap_fcntl(int fd, int cmd, ...) {
    int arg = 0;
    if (cmd == F_ADD_SEALS || cmd == F_SETFL || cmd == F_SETFD || cmd == F_DUPFD || cmd == F_DUPFD_CLOEXEC) {
        __builtin_va_list ap;
        __builtin_va_start(ap, cmd);
        arg = __builtin_va_arg(ap, int);
        __builtin_va_end(ap);
    }
#ifdef JNOX_GRAPHICS_OS
    if (jnox_tmp_fd(fd)) return jnox_tmp_fcntl(fd, cmd, arg);
#endif
    if (fd >= FD_BASE && fd < FD_BASE + FD_COUNT) {
        if (!enter()) return -1;
        struct descriptor *desc = &descriptors[fd - FD_BASE];
        if (!desc->kind) return finish(-EBADF);
        if (cmd == F_GETFL) return finish(O_RDWR | (desc->nonblock ? O_NONBLOCK : 0));
        if (cmd == F_SETFL) {
            if (arg & ~(O_ACCMODE | O_NONBLOCK)) return finish(-EINVAL);
            desc->nonblock = !!(arg & O_NONBLOCK);
            return finish(0);
        }
        if (cmd == F_GETFD) return finish(desc->cloexec ? FD_CLOEXEC : 0);
        if (cmd == F_SETFD && !(arg & ~FD_CLOEXEC)) {
            desc->cloexec = !!(arg & FD_CLOEXEC);
            return finish(0);
        }
        return finish(cmd == F_SETFD ? -EINVAL : -ENOTSUP);
    }
    if (fd < 0 && (cmd == F_DUPFD || cmd == F_DUPFD_CLOEXEC)) return error(EBADF);
    if (!backing_range(fd)) {
        return __real_fcntl(fd, cmd, arg);
    }
    if (cmd == F_DUPFD || cmd == F_DUPFD_CLOEXEC) {
        unsigned selector = cmd == F_DUPFD_CLOEXEC ? JNOX_SHM_DUP_CLOEXEC : JNOX_SHM_DUP_PLAIN;
        long value = jnox_ipc_trap(JNOX_CALL_SHM_DUP, fd, selector, (long)arg).status;
        return value < 0 ? error(-value) : (int)value;
    }
    if (cmd == F_ADD_SEALS) return jnox_shm_add_seals(fd, (unsigned)arg);
    if (cmd == F_GET_SEALS) return jnox_shm_get_seals(fd);
    // Close-on-exec belongs to each installed FD, including fixed backing.
    if (cmd == F_GETFD) return (int)backing_call(195, fd, 0);
    if (cmd == F_SETFD) return (int)backing_call(196, fd, arg);
    return jnox_shm_size(fd) < 0 ? -1 : error(ENOTSUP);
}

static long backing_io(int fd, unsigned operation, size_t length, uintptr_t buffer) {
    size_t n = length < JNOX_SHM_FILE_TRANSFER ? length : JNOX_SHM_FILE_TRANSFER;
    long r = jnox_ipc_trap(JNOX_CALL_SHM_FILE_IO, fd, (n << 8) | operation, buffer).status;
    return r < 0 ? error(-r) : r;
}

/* Validate the vector count and all lengths before a receive consumes bytes
 * or a send takes an in-flight reference. A stream call may transfer a prefix. */
static long prepare_message(struct transaction *t, unsigned op, int fd,
                            const struct msghdr *msg, int flags, int vector_io) {
    int allowed = op == OP_RECV_RIGHTS ? MSG_DONTWAIT | MSG_CMSG_CLOEXEC : MSG_DONTWAIT | MSG_NOSIGNAL;
    if (flags & ~allowed) return -ENOTSUP;
    if (!msg) return -EFAULT;
    if ((uintptr_t)msg % _Alignof(struct msghdr)) return -EINVAL;
    if (msg->msg_name || msg->msg_namelen) return -ENOTSUP;
    unsigned limit = vector_io ? JNOX_UIO_MAXIOV : JNOX_MSG_IOV_MAX;
    int size_error = vector_io ? -EINVAL : -EMSGSIZE;
    if (msg->msg_iovlen > limit) return size_error;
    if ((msg->msg_iovlen && !msg->msg_iov) ||
        (msg->msg_controllen && !msg->msg_control)) return -EFAULT;
    if (msg->msg_iovlen && (uintptr_t)msg->msg_iov % _Alignof(struct iovec)) return -EINVAL;
    if (msg->msg_controllen >= CMSG_LEN(sizeof(int)) &&
        (uintptr_t)msg->msg_control % _Alignof(struct cmsghdr)) return -EINVAL;
    size_t length = 0;
    for (size_t i = 0; i < msg->msg_iovlen; ++i) {
        const struct iovec *v = &msg->msg_iov[i];
        if (v->iov_len && !v->iov_base) return -EFAULT;
        if (v->iov_len > (size_t)__PTRDIFF_MAX__ - length) return size_error;
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

static ssize_t send_vectors(int fd, const struct msghdr *msg, int flags, int vector_io) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long n = prepare_message(&t, OP_SEND, fd, msg, flags, vector_io);
    if (n < 0) return finish(n);
    if (vector_io && n == 0) return finish(0);
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

static ssize_t recv_vectors(int fd, struct msghdr *msg, int flags, int vector_io) {
    if (!enter()) return -1;
    struct transaction t = {0};
    long n = prepare_message(&t, OP_RECV_RIGHTS, fd, msg, flags, vector_io);
    if (n < 0) return finish(n);
    if (vector_io && n == 0) return finish(0);
    t.arg = (unsigned)n;
    t.cloexec = !!(flags & MSG_CMSG_CLOEXEC);
    t.accept_right = msg->msg_controllen >= CMSG_LEN(sizeof(int));
    long r = commit(&t);
    return finish(r < 0 ? r : receive_message(msg, &t, r));
}

ssize_t sendmsg(int fd, const struct msghdr *msg, int flags) {
    return send_vectors(fd, msg, flags, 0);
}

ssize_t recvmsg(int fd, struct msghdr *msg, int flags) {
    return recv_vectors(fd, msg, flags, 0);
}

ssize_t writev(int fd, const struct iovec *iov, int count) {
    if (count < 0) return error(EINVAL);
    struct msghdr msg = { .msg_iov = (struct iovec *)iov, .msg_iovlen = (size_t)count };
    return send_vectors(fd, &msg, 0, 1);
}

ssize_t readv(int fd, const struct iovec *iov, int count) {
    if (count < 0) return error(EINVAL);
    struct msghdr msg = { .msg_iov = (struct iovec *)iov, .msg_iovlen = (size_t)count };
    return recv_vectors(fd, &msg, 0, 1);
}
