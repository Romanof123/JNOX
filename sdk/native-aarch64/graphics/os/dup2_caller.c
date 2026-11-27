#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <unistd.h>
#include "jnox/shm.h"

int main(void)
{
    int src = memfd_create("source", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    int dst = memfd_create("target", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (src != 96 || dst != 97) return 101;
    if (write(src, "abc", 3) != 3 || write(dst, "q", 1) != 1) return 102;
    int saved = fcntl(dst, F_DUPFD, 100);
    if (saved != 100 || lseek(src, 0, SEEK_SET) != 0 || lseek(dst, 0, SEEK_SET) != 0) return 103;

    if (dup2(dst, dst) != dst || fcntl(dst, F_GETFD) != FD_CLOEXEC) return 104;
    if (dup2(103, dst) != -1 || errno != EBADF || fcntl(dst, F_GETFD) != FD_CLOEXEC) return 105;
    char c;
    if (read(dst, &c, 1) != 1 || c != 'q' || lseek(saved, 0, SEEK_CUR) != 1) return 106;
    if (dup2(src, dst) != dst || fcntl(dst, F_GETFD) != 0 || fcntl(src, F_GETFD) != FD_CLOEXEC) return 107;
    if (read(dst, &c, 1) != 1 || c != 'a' || read(src, &c, 1) != 1 || c != 'b') return 108;
    if (jnox_shm_size(saved) != 1 || lseek(saved, 0, SEEK_SET) != 0 ||
        read(saved, &c, 1) != 1 || c != 'q') return 109;

    int ro = jnox_shm_dup_readonly(src);
    if (ro < 0 || dup2(ro, 103) != 103) return 110;
    if (write(103, "x", 1) != -1 || errno != EBADF || lseek(src, 0, SEEK_CUR) != 2) return 111;
    if (dup2(src, 104) != -1 || errno != EBADF) return 112;

    if (jnox_socket_broker(1)) return 113;
    int sock = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (sock != 64 || dup2(sock, sock) != sock) return 114;
    if (lseek(sock, 0, SEEK_CUR) != -1 || errno != ESPIPE) return 121;
    if (dup2(src, sock) != -1 || errno != ENOTSUP || fcntl(sock, F_GETFD) != FD_CLOEXEC) return 115;
    if (dup2(sock, dst) != -1 || errno != ENOTSUP || read(dst, &c, 1) != 1 || c != 'c') return 116;
    if (dup2(src, STDERR_FILENO) != -1 || errno != ENOTSUP) return 117;
    if (close(sock) || close(103) || close(ro) || close(saved) || close(src) || close(dst)) return 118;
    if (dup2(103, 103) != -1 || errno != EBADF) return 119;
    if (lseek(sock, 0, SEEK_CUR) != -1 || errno != EBADF ||
        lseek(80, 0, SEEK_CUR) != -1 || errno != EBADF ||
        lseek(104, 0, SEEK_CUR) != -1 || errno != EBADF) return 122;
    if (dup2(80, 80) != -1 || errno != EBADF) return 123;
    if (lseek(STDIN_FILENO, 0, SEEK_CUR) != -1 || errno != EBADF ||
        lseek(STDERR_FILENO, 0, SEEK_CUR) != -1 || errno != ESPIPE ||
        dup2(STDERR_FILENO, STDERR_FILENO) != STDERR_FILENO) return 124;
    /* Same zero-length fd 2 probe used before the OsInit fallback. */
    if (write(STDERR_FILENO, &c, 0) != 0) return 120;
    return 76;
}
