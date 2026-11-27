/* Guest checks for shared-file capability duplication. Included only by this fixture. */
#include <jnox/shared_fd_abi.h>
static unsigned char dup_observation[16];
#define DUP_CHECK(test) do { if (!(test)) return 0; } while (0)

static int duplicate_probe(void)
{
    int fd = memfd_create("dup", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    DUP_CHECK(fd == 96 && ftruncate(fd, 4096) == 0);
    // Upstream sends minimum zero. A lower bound below this provider still
    // selects its lowest allocatable slot. The source already occupies 96.
    int first = fcntl(fd, F_DUPFD_CLOEXEC, 0);
    DUP_CHECK(first == 97 && fcntl(first, F_GETFD) == FD_CLOEXEC && close(first) == 0);
    first = fcntl(fd, F_DUPFD, 95);
    DUP_CHECK(first == 97 && close(first) == 0);
    first = fcntl(fd, F_DUPFD, 96);
    DUP_CHECK(first == 97 && close(first) == 0);
    first = fcntl(fd, F_DUPFD_CLOEXEC, 103);
    DUP_CHECK(first == 103 && fcntl(fd, F_DUPFD, 103) == -1 && errno == EMFILE);
    DUP_CHECK(close(first) == 0);
    int high = fcntl(fd, F_DUPFD_CLOEXEC, 101);
    int low = fcntl(fd, F_DUPFD, 0);
    DUP_CHECK(high == 101 && low == 97);
    dup_observation[0] = high;
    dup_observation[1] = low;
    dup_observation[2] = fcntl(high, F_GETFD);
    dup_observation[3] = fcntl(low, F_GETFD);
    DUP_CHECK(dup_observation[2] == 1 && dup_observation[3] == 0);
    DUP_CHECK(fcntl(high, F_SETFD, 0) == 0 && fcntl(fd, F_GETFD) == 1);
    DUP_CHECK(fcntl(low, F_ADD_SEALS, F_SEAL_SHRINK) == 0);
    dup_observation[4] = fcntl(high, F_GET_SEALS);
    DUP_CHECK(dup_observation[4] == F_SEAL_SHRINK);
    DUP_CHECK(fcntl(-1, F_DUPFD_CLOEXEC, 0) == -1 && errno == EBADF);
    DUP_CHECK(fcntl(fd, F_DUPFD, -1) == -1 && errno == EINVAL);
    dup_observation[5] = errno;
    DUP_CHECK(fcntl(fd, F_DUPFD_CLOEXEC, 104) == -1 && errno == EINVAL);
    DUP_CHECK(jnox_ipc_trap(JNOX_CALL_SHM_DUP, fd, JNOX_SHM_DUP_RESERVED, 98).status == -EINVAL);
    int readonly = jnox_shm_dup_readonly(fd);
    int alias = fcntl(readonly, F_DUPFD_CLOEXEC, 98);
    DUP_CHECK(readonly == 98 && alias == 99 && close(readonly) == 0);
    DUP_CHECK(jnox_shm_map(alias, 3) == JNOX_SHM_MAP_FAILED && errno == EACCES);
    dup_observation[6] = errno;
    volatile unsigned char *map = jnox_shm_map(high, 3);
    DUP_CHECK(map != JNOX_SHM_MAP_FAILED);
    *map = 0xa5;
    DUP_CHECK(close(fd) == 0 && close(high) == 0 && close(low) == 0);
    DUP_CHECK(fcntl(low, F_DUPFD, 0) == -1 && errno == EBADF);
    dup_observation[7] = errno;
    DUP_CHECK(jnox_shm_size(alias) == 4096 && *map == 0xa5);
    int slots[7];
    for (int i = 0; i < 7; ++i) {
        slots[i] = fcntl(alias, F_DUPFD, 0);
        DUP_CHECK(slots[i] == 96 + i + (i >= 3));
    }
    DUP_CHECK(fcntl(alias, F_DUPFD_CLOEXEC, 97) == -1 && errno == EMFILE);
    dup_observation[8] = errno;
    DUP_CHECK(close(slots[2]) == 0);
    DUP_CHECK(fcntl(alias, F_DUPFD, 100) == -1 && errno == EMFILE);
    slots[2] = fcntl(alias, F_DUPFD_CLOEXEC, 98);
    dup_observation[9] = slots[2];
    DUP_CHECK(slots[2] == 98 && fcntl(slots[2], F_GETFD) == 1);
    for (int i = 0; i < 7; ++i) DUP_CHECK(close(slots[i]) == 0);
    DUP_CHECK(close(alias) == 0);
    dup_observation[10] = *map;
    DUP_CHECK(*map == 0xa5 && jnox_shm_unmap((void *)map) == 0);
    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    DUP_CHECK(sock >= 0 && fcntl(sock, F_DUPFD_CLOEXEC, 0) == -1 && errno == ENOTSUP);
    dup_observation[11] = errno;
    DUP_CHECK(close(sock) == 0 && fcntl(sock, F_DUPFD, 0) == -1 && errno == EBADF);
    dup_observation[12] = errno;
    return 1;
}

/* Emit actual slot/flag/errno/data observations after retirement of temp aliases. */
static void emit_dup_observations(void)
{
    char row[] = "JNOX DUPFD 00000000000000000000000000000000\n";
    const char digits[] = "0123456789abcdef";
    for (unsigned i = 0; i < sizeof(dup_observation); ++i) {
        row[11 + 2*i] = digits[dup_observation[i] >> 4];
        row[12 + 2*i] = digits[dup_observation[i] & 15];
    }
    write(1, row, sizeof(row)-1);
}
#undef DUP_CHECK
