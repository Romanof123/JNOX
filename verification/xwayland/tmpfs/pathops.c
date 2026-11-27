#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define CHECK(e, code) do { if (!(e)) return code; } while (0)
static int create(const char *p) { return open(p, O_CREAT | O_EXCL | O_RDWR, 0600); }

int main(int argc, char **argv)
{
    struct stat st, held;
    char c;
    if (argc > 1 && strcmp(argv[1], "ro") == 0) {
        CHECK(mkdir("/tmp/denied", 0700) == -1 && errno == EACCES, 101);
        CHECK(chmod("/tmp", 0777) == -1 && errno == EACCES, 102);
        int dir = open("/tmp", O_RDONLY);
        CHECK(dir >= 0 && fchmod(dir, 0777) == -1 && errno == EACCES && close(dir) == 0, 103);
        CHECK(link("/tmp/missing", "/tmp/new") == -1 && errno == EACCES, 104);
        return 89;
    }
    mode_t old = umask(0);
    CHECK(mkdir("/tmp/.X11-unix", 01777) == 0, 105);
    CHECK(mkdir("/tmp/.X11-unix", 01777) == -1 && errno == EEXIST, 106);
    CHECK(lstat("/tmp/.X11-unix", &st) == 0 && S_ISDIR(st.st_mode) && st.st_nlink == 2 &&
          (st.st_mode & 01777) == 01777 && st.st_uid == geteuid() && st.st_uid != 0, 107);
    int dir = open("/tmp/.X11-unix", O_RDONLY);
    CHECK(dir >= 0 && fstat(dir, &held) == 0 && st.st_ino == held.st_ino && st.st_dev == held.st_dev, 108);
    CHECK(fchmod(dir, 0600) == 0 && create("/tmp/.X11-unix/child") == -1 && errno == EACCES, 109);
    CHECK(chmod("/tmp/.X11-unix", 01777) == 0 && close(dir) == 0, 110);
    umask(old);

    /* Actual temporary-lock -> fchmod(0444) -> link -> unlink pattern. */
    int source = create("/tmp/.tX0-lock");
    CHECK(source >= 0 && write(source, "9", 1) == 1 && fchmod(source, 0444) == 0, 111);
    CHECK(link("/tmp/.tX0-lock", "/tmp/.X0-lock") == 0 && stat("/tmp/.X0-lock", &st) == 0 &&
          st.st_nlink == 2 && (st.st_mode & 0777) == 0444, 112);
    CHECK(link("/tmp/.tX0-lock", "/tmp/.X0-lock") == -1 && errno == EEXIST, 113);
    CHECK(unlink("/tmp/.tX0-lock") == 0 && fstat(source, &held) == 0 && held.st_nlink == 1 &&
          held.st_ino == st.st_ino, 114);
    CHECK(open("/tmp/.X0-lock", O_WRONLY) == -1 && errno == EACCES, 115);
    CHECK(link("/tmp/.X11-unix", "/tmp/dir-alias") == -1 && errno == EPERM, 116);
    CHECK(mkdir("/tmp/.X0-lock/bad", 0700) == -1 && errno == ENOTDIR, 117);
    int target = create("/tmp/old");
    CHECK(target >= 0 && write(target, "T", 1) == 1 && fstat(target, &st) == 0, 118);
    ino_t target_ino = st.st_ino;
    CHECK(rename("/tmp/.X0-lock", "/tmp/old") == 0 && stat("/tmp/.X0-lock", &st) == -1 && errno == ENOENT, 119);
    CHECK(stat("/tmp/old", &st) == 0 && st.st_ino == held.st_ino &&
          fstat(target, &st) == 0 && st.st_ino == target_ino && st.st_nlink == 0, 120);
    CHECK(lseek(target, 0, SEEK_SET) == 0 && read(target, &c, 1) == 1 && c == 'T', 121);
    CHECK(fsync(source) == -1 && errno == ENOTSUP && close(source) == 0 &&
          fsync(source) == -1 && errno == EBADF, 122);
    CHECK(close(target) == 0, 123);

    CHECK(mkdir("/tmp/a", 0700) == 0 && mkdir("/tmp/a/b", 0700) == 0 &&
          mkdir("/tmp/d", 0700) == 0 && stat("/tmp/a", &st) == 0 && st.st_nlink == 3, 124);
    CHECK(rename("/tmp/a", "/tmp/a/b/bad") == -1 && errno == EINVAL, 125);
    CHECK(rename("/tmp/d", "/tmp/a") == -1 && errno == ENOTEMPTY, 126);
    CHECK(rename("/tmp/old", "/tmp/a") == -1 && errno == EISDIR &&
          rename("/tmp/a", "/tmp/old") == -1 && errno == ENOTDIR, 127);
    dir = open("/tmp/d", O_RDONLY);
    CHECK(dir >= 0 && rename("/tmp/a", "/tmp/d") == 0 && stat("/tmp/d/b", &st) == 0 &&
          fstat(dir, &held) == 0 && held.st_nlink == 0 && close(dir) == 0, 128);
    CHECK(stat("/tmp/a", &st) == -1 && errno == ENOENT, 129);
    CHECK(rename("/tmp/old", "/tmp/d/b/f") == 0 && link("/tmp/d/b/f", "/tmp/alias") == 0, 130);
    CHECK(rename("/tmp/alias", "/tmp/d/b/f") == 0 && stat("/tmp/alias", &st) == 0 && st.st_nlink == 2, 131);
    CHECK(chmod("/tmp/d", 0500) == 0 && rename("/tmp/d/b", "/tmp/moved") == -1 && errno == EACCES, 132);
    CHECK(chmod("/tmp/d", 0700) == 0 && rename("/tmp/d/b", "/tmp/moved") == 0 &&
          stat("/tmp/moved/f", &st) == 0, 133);
    CHECK(chmod("/", 0777) == -1 && errno == EROFS && unlink("/tmp/moved") == -1 && errno == EISDIR, 134);
    /* Fill remaining directory entries with links to one inode. */
    char name[] = "/tmp/k0"; unsigned created = 0;
    for (; created < 10; ++created) {
        name[6] = '0' + created;
        if (link("/tmp/moved/f", name) < 0) break;
    }
    CHECK(created > 0 && created < 10 && errno == ENOSPC && stat("/tmp/alias", &st) == 0 &&
          st.st_nlink == 2 + created, 135);
    CHECK(rename("/tmp/alias", "/tmp/renamed") == 0 && stat("/tmp/renamed", &st) == 0, 136);
    return 89;
}
