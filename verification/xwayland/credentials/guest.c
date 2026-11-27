#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <unistd.h>

#ifndef EXPECT_UID
#define EXPECT_UID 65534
#endif
#ifndef EXPECT_GID
#define EXPECT_GID 65534
#endif

int main(void)
{
    const uid_t uid = getuid();
    const gid_t gid = getgid();
    if (uid != EXPECT_UID || gid != EXPECT_GID) return 101;
    if (geteuid() != uid || getegid() != gid) return 102;
    errno = 0;
    if (seteuid(0) != -1 || errno != EPERM) return 103;
    errno = 0;
    if (seteuid(uid == 1 ? 2 : 1) != -1 || errno != EPERM) return 104;
    errno = 0;
    if (seteuid((uid_t)-1) != -1 || errno != EINVAL) return 105;
    errno = EBUSY;
    if (seteuid(uid) != 0 || errno != EBUSY) return 106;
    if (getuid() != uid || geteuid() != uid || getgid() != gid || getegid() != gid) return 107;
    return 84;
}
