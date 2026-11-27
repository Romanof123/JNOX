/* Select "m" in the ordinary #444 producer slot. All existing roles and
 * wire-level acceptance gates are exercised by their normal entry points. */
#include "probe.h"
#include <sys/socket.h>
#include <jnox/ipc.h>
#include "dup_probe.h"
static int duplicate_frame;
static int create_shared_frame;
static int receiver;
static int receive_cloexec;

static int frame_backing(size_t length)
{
    if (!create_shared_frame || length != JNOX_SHM_MAX_BYTES) return jnox_shm_create(length);
    create_shared_frame = 0;
    int fd = memfd_create("xwayland-shared", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (fd < 0) return -1;
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK) || ftruncate(fd, (off_t)length)) {
        jnox_shm_close(fd);
        return -1;
    }
    jnox_xwl_shm_probe[3] = 1;
    return fd;
}

static long frame_size(int fd)
{
    long size = jnox_shm_size(fd);
    if (receiver && size == JNOX_SHM_MAX_BYTES) {
        if (fcntl(fd, F_GETFD) != receive_cloexec || fcntl(fd, F_GET_SEALS) != F_SEAL_SHRINK ||
            jnox_shm_resize(fd, 1) != -1 || errno != EACCES) return -1;
        jnox_xwl_shm_probe[4] = 1;
        jnox_xwl_shm_probe[6] = (unsigned)receive_cloexec;
    }
    return size;
}

extern long long volatile fd_transfer_witness[32];
static ssize_t receive_frame(int fd, struct msghdr *msg, int flags)
{
    /* Stage 10 has queued bytes and an ancillary right. Test refusal before
     * the normal receiver checks the original prefix and installs the FD. */
    if (receiver && fd_transfer_witness[1] == 10 && !jnox_xwl_shm_probe[5]) {
        if (recvmsg(fd, msg, flags | 0x1000) != -1 || errno != ENOTSUP) return -1;
        jnox_xwl_shm_probe[5] = 1;
    }
    return recvmsg(fd, msg, flags | (receive_cloexec ? MSG_CMSG_CLOEXEC : 0));
}

static ssize_t send_frame(int fd, const struct msghdr *msg, int flags)
{
    if (sendmsg(fd, msg, flags | MSG_CMSG_CLOEXEC) != -1 || errno != ENOTSUP) return -1;
    jnox_xwl_shm_probe[7] = 1;
    return sendmsg(fd, msg, flags);
}

static jnox_ipc_response fixture_call(unsigned peer, uint64_t word, uint64_t deadline)
{
    /* The ordinary cancellation fixture uses raw BEGIN so that only COMMIT
     * gets a deadline. Keep its receive flag consistent with this run. */
    if (receive_cloexec && (word >> 48) == 0x1110) word |= 1ULL << 10;
    return jnox_ipc_call(peer, word, deadline);
}

static int frame_readonly(int fd)
{
    int original = jnox_shm_dup_readonly(fd);
    if (!duplicate_frame || original < 0) return original;
    int alias = fcntl(original, F_DUPFD_CLOEXEC, 0);
    if (alias >= 0 && jnox_shm_size(fd) == JNOX_SHM_MAX_BYTES) {
        dup_observation[13] = original;
        dup_observation[14] = alias;
        dup_observation[15] = fcntl(alias, F_GETFD);
        emit_dup_observations();
    }
    jnox_shm_close(original);
    return alias;
}
#define jnox_shm_dup_readonly frame_readonly
#define jnox_ipc_call fixture_call
#define sendmsg send_frame
#define recvmsg receive_frame
#define jnox_shm_create frame_backing
#define jnox_shm_size frame_size
#define main ordinary_transfer_main
#include "../native-fd-transfer/main.c"
#undef main

int main(int argc, char **argv)
{
    if (argc == 2 && argv[1] && (argv[1][0] == 'm' || argv[1][0] == 'd') && !argv[1][1]) {
        duplicate_frame = argv[1][0] == 'd';
        if (jnox_socket_broker(3) || !shared_file_probe()) return 104;
        if (duplicate_frame && !duplicate_probe()) return 105;
        create_shared_frame = 1;
        W[ROLE] = 1;
        return producer();
    }
    if (argc == 2 && argv[1] && argv[1][0] == 'c' && !argv[1][1]) {
        receiver = receive_cloexec = 1;
        return ordinary_transfer_main(1, argv);
    }
    receiver = argc == 1;
    return ordinary_transfer_main(argc, argv);
}
