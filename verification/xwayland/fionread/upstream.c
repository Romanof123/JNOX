/* Build in the same target sysroot and Unix-only transport profile as Xlib. */
#define X11_t
#define XTRANS_TRANSPORT_C
#define TRANS_CLIENT
#define UNIXCONN
#include <X11/Xtrans/Xtransint.h>
#include <sys/ioctl.h>
#include <sys/un.h>
#include <stddef.h>
#include <string.h>
#include <unistd.h>

#include "original-socket.inc"

_Static_assert(sizeof(BytesReadable_t) == sizeof(int), "target Xtrans FIONREAD ABI");

static int pending(int fd, int expected)
{
    struct _XtransConnInfo connection = { .fd = fd };
    BytesReadable_t bytes = -1;
    return TRANS(SocketBytesReadable)(&connection, &bytes) == 0 && bytes == expected;
}

static int verify_vectors(int sender, int receiver)
{
    struct _XtransConnInfo tx = { .fd = sender }, rx = { .fd = receiver };
    char source[72], received[72];
    for (size_t i = 0; i < sizeof(source); ++i) source[i] = (char)(1 + i);
    memset(received, 0, sizeof(received));
    struct iovec outgoing[JNOX_UIO_MAXIOV] = {
        [0] = {source, 7}, [JNOX_UIO_MAXIOV - 1] = {source + 7, sizeof(source) - 7}
    };
    struct iovec incoming[] = {{received, 3}, {NULL, 0}, {received + 3, sizeof(received) - 3}};
    struct iovec overflow[] = {{source, (size_t)__PTRDIFF_MAX__}, {source, 1}};
    if (writev(sender, NULL, -1) != -1 || errno != EINVAL ||
        writev(sender, outgoing, JNOX_UIO_MAXIOV + 1) != -1 || errno != EINVAL ||
        writev(sender, overflow, 2) != -1 || errno != EINVAL ||
        writev(sender, NULL, 0) || readv(receiver, NULL, 0) || !pending(receiver, 0)) return 21;
    int sent = TRANS(SocketWritev)(&tx, outgoing, JNOX_UIO_MAXIOV);
    if (sent <= 7 || (size_t)sent >= sizeof(source) || !pending(receiver, sent)) return 22;
    struct iovec invalid[] = {{received, 1}, {NULL, 1}};
    if (readv(receiver, invalid, 2) != -1 || errno != EFAULT ||
        readv(receiver, overflow, 2) != -1 || errno != EINVAL ||
        received[0] || !pending(receiver, sent)) return 23;
    if (TRANS(SocketReadv)(&rx, incoming, 3) != sent ||
        memcmp(source, received, sent) || received[sent] || !pending(receiver, 0)) return 24;
    /* Retry from the reported offset exactly as a stream caller must. */
    struct iovec tail = {source + sent, sizeof(source) - sent};
    struct iovec dst = {received + sent, sizeof(received) - sent};
    if (TRANS(SocketWritev)(&tx, &tail, 1) != (int)tail.iov_len ||
        TRANS(SocketReadv)(&rx, &dst, 1) != (int)dst.iov_len ||
        memcmp(source, received, sizeof(source))) return 25;
    return 0;
}

/* Package as slot 0 with a local AF_UNIX broker in slot 1, live mask 1. */
int main(void)
{
    if (jnox_socket_broker(1) < 0) return 10;
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    int sender = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un path = { .sun_family = AF_UNIX, .sun_path = "/count" };
    socklen_t len = offsetof(struct sockaddr_un, sun_path) + sizeof("/count");
    if (bind(listener, (struct sockaddr *)&path, len) || listen(listener, 1) ||
        connect(sender, (struct sockaddr *)&path, len)) return 11;
    int receiver = accept(listener, NULL, NULL);
    if (receiver < 0 || !pending(receiver, 0)) return 12;
    int vector_result = verify_vectors(sender, receiver);
    if (vector_result) return vector_result;
    if (send(sender, "abcde", 5, 0) != 5 || !pending(receiver, 5) || !pending(receiver, 5)) return 13;
    char buf[5];
    if (recv(receiver, buf, 2, 0) != 2 || !pending(receiver, 3) || close(sender) ||
        !pending(receiver, 3)) return 14;
    if (recv(receiver, buf + 2, 3, 0) != 3 || memcmp(buf, "abcde", 5) ||
        !pending(receiver, 0)) return 15;
    struct iovec eof = {buf, 1};
    if (readv(receiver, &eof, 1) || close(receiver)) return 16;
    if (readv(receiver, &eof, 1) != -1 || errno != EBADF) return 26;
    int result = -9;
    if (ioctl(receiver, FIONREAD, &result) != -1 || errno != EBADF || result != -9) return 17;
    if (ioctl(listener, FIONREAD, &result) != -1 || errno != EINVAL || result != -9) return 18;
    if (ioctl(listener, FIONREAD, NULL) != -1 || errno != EFAULT) return 19;
    if (ioctl(listener, 0, &result) != -1 || errno != ENOTTY) return 20;
    return close(listener) != 0;
}
