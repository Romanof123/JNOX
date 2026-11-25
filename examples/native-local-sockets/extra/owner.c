#include <errno.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <jnox/ipc.h>

static const struct sockaddr_un name = { AF_UNIX, "/extra" };
/* deadline, COMMIT transport status and reply, completion time, reads/markers,
   final empty status, and raw META/READ for successful finite receives. */
volatile uint64_t extra_owner_result[16];

static uint64_t now(void)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return 0;
    return (uint64_t)t.tv_sec * 1000000000u + t.tv_nsec;
}

static int pause_until(uint64_t time)
{
    struct timespec t = { (long)(time / 1000000000u), (long)(time % 1000000000u) };
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0);
}

static int peer_start(uint64_t deadline)
{
    jnox_ipc_response r = jnox_ipc_call(1, deadline, UINT64_MAX);
    return r.status == 0 && r.word == 0xa1;
}

static int peer_done(void)
{
    jnox_ipc_request r = jnox_ipc_receive();
    return r.status == 0 && r.word == 0xa2 && jnox_ipc_reply(r.token, 0xa3) == 0;
}

static int backlog(void)
{
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener < 0 || bind(listener, (const struct sockaddr *)&name, sizeof name) || listen(listener, 2)) return 91;
    uint64_t at = now();
    if (!at || !peer_start(at) || pause_until(at + 100000000)) return 92;
    for (unsigned i = 0; i < 3; ++i) {
        unsigned char marker = 0;
        extra_owner_result[10+i] = now();
        int accepted = accept(listener, 0, 0);
        if (accepted < 0 || recv(accepted, &marker, 1, 0) != 1 || marker != 51+i) return 93;
        extra_owner_result[4+i] = marker;
        if (close(accepted)) return 94;
    }
    return peer_done() && close(listener) == 0 ? 81 : 95;
}

static int datagram(int expire)
{
    int fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if (fd < 0 || bind(fd, (const struct sockaddr *)&name, sizeof name)) return 96;
    // SDK descriptors 64..67 map to broker wire slots 0..3.
    if (fd < 64 || fd >= 68) return 96;
    uint64_t begin = ((uint64_t)0x11 << 56) | ((uint64_t)7 << 48) | ((uint64_t)(fd-64) << 40) | ((uint64_t)1 << 32);
    jnox_ipc_response staged = jnox_ipc_call(2, begin, UINT64_MAX);
    if (staged.status || staged.word) return 97;
    uint64_t start = now();
    uint64_t deadline = start + 200000000;
    extra_owner_result[0] = deadline;
    if (!start || !peer_start(deadline)) return 98;
    jnox_ipc_response commit = jnox_ipc_call(2, (uint64_t)0x13 << 56, deadline);
    extra_owner_result[1] = (uint64_t)commit.status;
    extra_owner_result[2] = commit.status ? 0 : commit.word;
    extra_owner_result[3] = now();
    if (expire ? (commit.status != -110 || extra_owner_result[3] < deadline) :
        (commit.status != 0 || commit.word != 1 || extra_owner_result[3] >= deadline)) return 99;

    // Receive only from the peer's direct control endpoint until its send returns.
    // In particular do not send ABORT, BEGIN, META or READ to the broker here.
    if (!peer_done()) return 100;
    unsigned char byte = 0;
    if (expire) {
        ssize_t n = recv(fd, &byte, 1, MSG_DONTWAIT);
        extra_owner_result[4] = (uint64_t)n;
        extra_owner_result[10] = n < 0 ? (unsigned)errno : 0;
        extra_owner_result[5] = byte;
        if (n != 1 || byte != 0x6d) return 101;
    } else {
        jnox_ipc_response meta = jnox_ipc_call(2, (uint64_t)0x14 << 56, UINT64_MAX);
        jnox_ipc_response data = jnox_ipc_call(2, (uint64_t)0x15 << 56, UINT64_MAX);
        extra_owner_result[8] = meta.word;
        extra_owner_result[9] = data.word;
        if (meta.status || (meta.word & 0xffff) != 0x0101 || data.status || (data.word & 255) != 0x6d) return 102;
    }
    ssize_t n = recv(fd, &byte, 1, MSG_DONTWAIT);
    extra_owner_result[6] = (uint64_t)n;
    extra_owner_result[7] = errno;
    return n == -1 && errno == EAGAIN && close(fd) == 0 ? 81 : 103;
}

int main(int argc, char **argv)
{
    if (argc != 2 || jnox_socket_broker(2)) return 90;
    if (argv[1][0] == 'b') return backlog();
    if (argv[1][0] == 'e' || argv[1][0] == 'r') return datagram(argv[1][0] == 'e');
    return 104;
}
