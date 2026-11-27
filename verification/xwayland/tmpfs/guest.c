#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define CHECK(e, code) do { if (!(e)) return code; } while (0)
static int create(const char *name) { return open(name, O_RDWR | O_CREAT | O_EXCL, 0666); }

int main(int argc, char **argv)
{
    struct stat st;
    char buf[8] = {0};
    const char *mode = argc > 1 ? argv[1] : "rw";
    if (strcmp(mode, "denied") == 0) {
        CHECK(open("/tmp/file", O_RDONLY) == -1 && errno == EACCES, 101);
        CHECK(stat("/tmp", &st) == -1 && errno == EACCES, 102);
        return 88;
    }
    if (strcmp(mode, "ro") == 0) {
        CHECK(create("/tmp/forbidden") == -1 && errno == EACCES, 103);
        CHECK(stat("/tmp", &st) == 0 && S_ISDIR(st.st_mode), 104);
        CHECK(open("/tmp/missing", O_RDONLY) == -1 && errno == ENOENT, 105);
        return 88;
    }
    if (strcmp(mode, "reader") == 0) {
        int fd = open("/tmp/share", O_RDONLY);
        CHECK(fd >= 32 && fd < 40, 106);
        CHECK(read(fd, buf, 3) == 3 && memcmp(buf, "abc", 3) == 0, 107);
        CHECK(fstat(fd, &st) == 0 && st.st_size == 3 && st.st_uid == getuid(), 108);
        CHECK(unlink("/tmp/share") == -1 && errno == EACCES && close(fd) == 0, 109);
        return 88;
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
