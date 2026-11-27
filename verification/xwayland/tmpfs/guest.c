#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

#define CHECK(e, code) do { if (!(e)) return code; } while (0)
static int create(const char *name) { return open(name, O_RDWR | O_CREAT | O_EXCL, 0666); }

static int x11_bind_as_role(int allowed)
{
    if (jnox_socket_broker(3) < 0) return 0;
    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock < 0) return 0;
    struct sockaddr_un addr = {AF_UNIX, "/tmp/.X11-unix/X0"};
    int rc = bind(sock, (struct sockaddr *)&addr, sizeof(addr));
    int ok = allowed ? (rc == 0 && listen(sock, 1) == 0) : (rc == -1 && errno == EACCES);
    return close(sock) == 0 && ok;
}

int main(int argc, char **argv)
{
    struct stat st;
    char buf[8] = {0};
    const char *mode = argc > 1 ? argv[1] : "rw";
    int x11 = strncmp(mode, "x11-", 4) == 0;
    if (x11) mode += 4;
    if (strcmp(mode, "cold") == 0) {
        const char *name = "/tmp/cold-marker";
        CHECK(stat(name, &st) == -1 && errno == ENOENT, 145);
        int fd = create(name);
        CHECK(fd >= 0 && write(fd, "JNOX", 4) == 4 && close(fd) == 0, 146);
        fd = open(name, O_RDONLY);
        CHECK(fd >= 0 && read(fd, buf, 4) == 4 && memcmp(buf, "JNOX", 4) == 0, 147);
        CHECK(close(fd) == 0 && stat(name, &st) == 0 && st.st_size == 4, 148);
        return 88; /* Leave the named file present through this boot's final stat. */
    }
    if (strcmp(mode, "denied") == 0) {
        CHECK(open("/tmp/file", O_RDONLY) == -1 && errno == EACCES, 101);
        CHECK(stat("/tmp", &st) == -1 && errno == EACCES, 102);
        if (x11) CHECK(x11_bind_as_role(0), 149);
        return 88;
    }
    if (strcmp(mode, "ro") == 0) {
        CHECK(create("/tmp/forbidden") == -1 && errno == EACCES, 103);
        CHECK(stat("/tmp", &st) == 0 && S_ISDIR(st.st_mode), 104);
        CHECK(open("/tmp/missing", O_RDONLY) == -1 && errno == ENOENT, 105);
        return 88;
    }
    if (strcmp(mode, "reader") == 0) {
        if (x11) CHECK(lstat("/tmp/.X11-unix", &st) == 0 && st.st_uid == 0 && st.st_gid == 0 &&
              S_ISDIR(st.st_mode) && (st.st_mode & 01777) == 01777 && x11_bind_as_role(0), 150);
        int fd = open("/tmp/share", O_RDONLY);
        CHECK(fd >= 32 && fd < 40, 106);
        CHECK(read(fd, buf, 3) == 3 && memcmp(buf, "abc", 3) == 0, 107);
        CHECK(fstat(fd, &st) == 0 && st.st_size == 3 && st.st_uid == getuid(), 108);
        CHECK(unlink("/tmp/share") == -1 && errno == EACCES && close(fd) == 0, 109);
        return 88;
    }
    if (x11 && strcmp(mode, "writer") == 0) {
        const char *dir = "/tmp/.X11-unix";
        CHECK(geteuid() != 0 && lstat(dir, &st) == 0 && S_ISDIR(st.st_mode) &&
              (st.st_mode & 01777) == 01777 && st.st_uid == 0 && st.st_gid == 0, 151);
        ino_t ino = st.st_ino;
        int d = open(dir, O_RDONLY);
        CHECK(d >= 0 && fstat(d, &st) == 0 && st.st_ino == ino && st.st_uid == 0 &&
              fchmod(d, 0777) == -1 && errno == EPERM && close(d) == 0, 152);
        CHECK(chmod("/tmp", 0700) == -1 && errno == EPERM &&
              chmod(dir, 0777) == -1 && errno == EPERM, 153);
        CHECK(mkdir(dir, 01777) == -1 && errno == EEXIST &&
              rename(dir, "/tmp/moved") == -1 && errno == EPERM, 154);
        CHECK(mkdir("/tmp/replacement", 0700) == 0 && rename("/tmp/replacement", dir) == -1 &&
              errno == EPERM && create("/tmp/.X11-unix/X0") == -1 && errno == EACCES, 155);
        CHECK(unlink("/tmp/.X11-unix/X0") == -1 && errno == ENOENT && x11_bind_as_role(1), 156);
    }
    if (strcmp(mode, "writer") == 0) {
        int f = create("/tmp/share"), gone = create("/tmp/unlinked");
        CHECK(f >= 0 && gone >= 0 && write(f, "abc", 3) == 3 && unlink("/tmp/unlinked") == 0, 110);
        return 88; /* The broker receives the client exit and retires both descriptions. */
    }
    if (strcmp(mode, "unset") == 0) {
        int f = create("/tmp/untimed");
        CHECK(f >= 0 && write(f, "abc", 3) == 3, 111);
        CHECK(fstat(f, &st) == -1 && errno == ENODATA && close(f) == 0, 112);
        return 88;
    }
    CHECK(strcmp(mode, "rw") == 0, 113);
    CHECK(stat("/tmp", &st) == 0 && S_ISDIR(st.st_mode) && st.st_mtim.tv_sec >= 1700000000, 114);
    CHECK(stat("/tmp/alpha", &st) == -1 && errno == ENOENT, 115);
    CHECK(create("/immutable") == -1 && errno == EROFS, 116);
    CHECK(create("/tmp/../leak") == -1 && errno == EINVAL, 117);
    CHECK(create("/tmp//alpha") == -1 && errno == EINVAL, 118);
    CHECK(umask(027) == 022, 119);
    int a = create("/tmp/alpha");
    CHECK(a == 32 && create("/tmp/alpha") == -1 && errno == EEXIST, 120);
    int b = open("/tmp/alpha", O_RDWR), dupfd = fcntl(a, F_DUPFD_CLOEXEC, 0);
    CHECK(b == 33 && dupfd == 34 && fcntl(dupfd, F_GETFD) == FD_CLOEXEC, 121);
    CHECK(write(a, "abcd", 4) == 4 && lseek(dupfd, 0, SEEK_CUR) == 4 && lseek(b, 0, SEEK_CUR) == 0, 122);
    CHECK(read(b, buf, 2) == 2 && memcmp(buf, "ab", 2) == 0, 123);
    CHECK(lseek(a, 1, SEEK_END) == 5 && write(dupfd, "Z", 1) == 1, 124);
    CHECK(read(b, buf, 4) == 4 && memcmp(buf, "cd\0Z", 4) == 0, 125);
    CHECK(fstat(a, &st) == 0 && S_ISREG(st.st_mode) && (st.st_mode & 0777) == 0640 &&
          st.st_size == 6 && st.st_uid == getuid() && st.st_gid == getgid() && st.st_nlink == 1, 126);
    ino_t original = st.st_ino;
    CHECK(lstat("/tmp/alpha", &st) == 0 && st.st_ino == original, 127);
    CHECK(unlink("/tmp/alpha") == 0 && stat("/tmp/alpha", &st) == -1 && errno == ENOENT, 128);
    CHECK(fstat(dupfd, &st) == 0 && st.st_ino == original && st.st_nlink == 0, 129);
    int fresh = create("/tmp/alpha");
    CHECK(fresh >= 0 && fstat(fresh, &st) == 0 && st.st_ino != original && st.st_size == 0, 130);
    CHECK(dup2(dupfd, dupfd) == dupfd && fcntl(dupfd, F_GETFD) == FD_CLOEXEC, 138);
    CHECK(dup2(39, dupfd) == -1 && errno == EBADF && fcntl(dupfd, F_GETFD) == FD_CLOEXEC, 139);
    CHECK(dup2(39, 96) == -1 && errno == EBADF, 140);
    CHECK(dup2(fresh, dupfd) == dupfd && fcntl(dupfd, F_GETFD) == 0 &&
          write(dupfd, "k", 1) == 1 && lseek(fresh, 0, SEEK_CUR) == 1, 141);
    CHECK(fstat(dupfd, &st) == 0 && st.st_ino != original && st.st_size == 1 &&
          fstat(b, &st) == 0 && st.st_ino == original && st.st_size == 6, 142);
    CHECK(dup2(fresh, 96) == -1 && errno == ENOTSUP && lseek(fresh, 0, SEEK_CUR) == 1, 143);
    CHECK(close(a) == 0 && close(dupfd) == 0 && close(b) == 0, 131);
    CHECK(lseek(dupfd, 0, SEEK_CUR) == -1 && errno == EBADF, 144);
    CHECK(lseek(fresh, 1023, SEEK_SET) == 1023 && write(fresh, "xx", 2) == 1 &&
          write(fresh, "x", 1) == -1 && errno == ENOSPC && lseek(fresh, 0, SEEK_CUR) == 1024, 132);
    CHECK(close(fresh) == 0 && fstat(fresh, &st) == -1 && errno == EBADF, 133);
    const char *names[] = {"/tmp/b", "/tmp/c", "/tmp/d"};
    for (unsigned i = 0; i < 3; ++i) { int fd = create(names[i]); CHECK(fd >= 0 && close(fd) == 0, 134); }
    CHECK(create("/tmp/e") == -1 && errno == ENOSPC, 135);
    CHECK(unlink("/tmp/c") == 0, 136);
    int e = open("/tmp/e", O_WRONLY|O_CREAT, 0200);
    CHECK(e >= 0 && close(e) == 0 && open("/tmp/e", O_RDONLY) == -1 && errno == EACCES, 137);
    return 88;
}
