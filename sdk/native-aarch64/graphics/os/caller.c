/* Pair with a PID 3 local-socket broker and a peer serving /wayland-os
 * that reads one byte, returns that byte plus one, and closes the stream. */
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <stddef.h>
#include <sys/un.h>
#include <unistd.h>
int main(void)
{
    pthread_mutex_t mutex;
    pthread_cond_t condition;
    if (pthread_mutex_init(&mutex, NULL) || pthread_cond_init(&condition, NULL) ||
        pthread_mutex_lock(&mutex) || pthread_cond_signal(&condition) ||
        pthread_cond_broadcast(&condition) ||
        pthread_mutex_unlock(&mutex) || pthread_cond_destroy(&condition) ||
        pthread_mutex_destroy(&mutex)) return 10;
    if (jnox_socket_broker(3)) return 11;
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0 || fcntl(fd, F_GETFD) != FD_CLOEXEC) return 12;
    struct sockaddr_un address = {AF_UNIX, "/wayland-os"};
    if (connect(fd, (struct sockaddr *)&address,
                offsetof(struct sockaddr_un, sun_path) + sizeof("/wayland-os"))) return 13;
    unsigned char byte = 0x55;
    struct iovec iov = {&byte, 1};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1};
    if (sendmsg(fd, &msg, MSG_NOSIGNAL | MSG_DONTWAIT) != 1) return 14;
    struct pollfd ready = {fd, POLLIN, 0};
    if (poll(&ready, 1, 1000) != 1 || !(ready.revents & POLLIN)) return 15;
    if (recvmsg(fd, &msg, MSG_DONTWAIT | MSG_CMSG_CLOEXEC) != 1 || byte != 0x56) return 16;
    if (fcntl(fd, F_SETFD, 0) || fcntl(fd, F_GETFD) || close(fd)) return 17;
    if (fcntl(fd, F_GETFD) != -1 || errno != EBADF) return 18;
    return 0;
}
