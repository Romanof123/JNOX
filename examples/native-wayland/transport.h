/* Bounded client transport for little-endian native AArch64. */
#ifndef CLIENT_TRANSPORT_H
#define CLIENT_TRANSPORT_H
#include <stdint.h>
#include <sys/un.h>

static int channel;
static int queued_fds[4];
static unsigned queued_count;
union control_buffer {
    struct cmsghdr align;
    unsigned char bytes[CMSG_SPACE(sizeof(int))];
};

static int send_wire(unsigned object, unsigned opcode, const uint32_t *args, unsigned count, int pass_fd)
{
    uint32_t words[32];
    if (count > 30) return 0;
    unsigned size = (count + 2) * 4;
    words[0] = object;
    words[1] = (size << 16) | opcode;
    for (unsigned i = 0; i < count; ++i) words[i + 2] = args[i];
    unsigned sent = 0;
    if (pass_fd >= 0) {
        union control_buffer control = {0};
        struct iovec vec = {words, size};
        struct msghdr msg = {0};
        msg.msg_iov = &vec;
        msg.msg_iovlen = 1;
        msg.msg_control = control.bytes;
        msg.msg_controllen = sizeof(control.bytes);
        struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
        c->cmsg_len = CMSG_LEN(sizeof(pass_fd));
        c->cmsg_level = SOL_SOCKET;
        c->cmsg_type = SCM_RIGHTS;
        __builtin_memcpy(CMSG_DATA(c), &pass_fd, sizeof(pass_fd));
        ssize_t n = sendmsg(channel, &msg, 0);
        if (n <= 0 || (unsigned)n > size) return 0;
        sent = (unsigned)n;
    }
    while (sent < size) {
        ssize_t n = send(channel, (unsigned char *)words + sent, size - sent, 0);
        if (n <= 0) return 0;
        sent += (unsigned)n;
    }
    return 1;
}

#define WORD_REQUEST(object, opcode, ...) \
    send_wire((object), (opcode), (uint32_t[]){__VA_ARGS__}, \
              sizeof((uint32_t[]){__VA_ARGS__}) / sizeof(uint32_t), -1)

static unsigned pack_string(uint32_t *out, unsigned room, const char *s)
{
    unsigned length = 0;
    while (s[length]) ++length;
    ++length; /* includes the NUL, excludes padding */
    unsigned cells = (length + 3) / 4;
    if (1 + cells > room) return 0;
    out[0] = length;
    for (unsigned i = 0; i < cells; ++i) out[1+i] = 0;
    for (unsigned i = 0; i < length; ++i) ((char *)(out+1))[i] = s[i];
    return 1 + cells;
}

static int string_request(unsigned object, unsigned opcode, const char *s)
{
    uint32_t arg[16];
    unsigned n = pack_string(arg, 16, s);
    return n && send_wire(object, opcode, arg, n, -1);
}

static int read_wire(void *dest, unsigned size)
{
    unsigned done = 0;
    while (done < size) {
        union control_buffer control = {0};
        struct iovec vec = {(unsigned char *)dest + done, size - done};
        struct msghdr msg = {0};
        msg.msg_iov = &vec;
        msg.msg_iovlen = 1;
        msg.msg_control = control.bytes;
        msg.msg_controllen = sizeof(control.bytes);
        ssize_t n = recvmsg(channel, &msg, 0);
        if (n <= 0 || (msg.msg_flags & MSG_CTRUNC)) return 0;
        if (msg.msg_controllen) {
            struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
            if (!c || c->cmsg_len != CMSG_LEN(sizeof(int)) || c->cmsg_level != SOL_SOCKET ||
                c->cmsg_type != SCM_RIGHTS || queued_count == 4) return 0;
            __builtin_memcpy(&queued_fds[queued_count++], CMSG_DATA(c), sizeof(int));
        }
        done += (unsigned)n;
    }
    return 1;
}

static int take_fd(void)
{
    if (!queued_count) return -1;
    int fd = queued_fds[0];
    --queued_count;
    for (unsigned i = 0; i < queued_count; ++i) queued_fds[i] = queued_fds[i+1];
    return fd;
}
#endif
